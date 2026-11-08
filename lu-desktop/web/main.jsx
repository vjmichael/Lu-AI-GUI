import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { splitModelOutput } from './response_view.mjs';
import { fullEditChangeLines } from './edit_diff.mjs';
import { MarkdownView } from './markdown_view.jsx';
import {buildSubmittedPrompt, attachmentSummary, MAX_TEXT_FILES} from './attachment_drafts.mjs';
import {isPotentialImage, normalizedImageFile} from './image_attachment_input.mjs';
import {stagedImageDimensions,REGION_IMAGE_OPERATIONS,
  imagePointerPixel,imageDragRectangle} from './image_assembly_geometry.mjs';
import {queuedJob, startedJob, tokenForJob, removeJob} from './generation_jobs.mjs';
import {nearTranscriptBottom, restoredScrollTop, isSendKey, isSearchKey, isNewChatKey, matchesChatTitle} from './chat_ui_behavior.mjs';
import {MAX_HISTORY_ROWS,mergeHistoryWindow,captureHistoryAnchor,restoreHistoryAnchor} from './history_window_geometry.mjs';

function MessageText({text, role='assistant'}) {
  if (role === 'user') return <div className="messageContent userText">{text}</div>;
  return <div className="messageContent">{splitModelOutput(text).map((part,i) =>
    part.kind === 'think' ? <details key={i} className="thinkBlock"><summary>Model reasoning</summary><MarkdownView text={part.text}/></details>
      : <MarkdownView key={i} text={part.text}/>)}</div>;
}
const MAX_TEXT_ATTACHMENT = 64 * 1024;
const MAX_IMAGE_ATTACHMENT = 8 * 1024 * 1024;


/**
 * Independent human ANSF history reader. Does not call setChatId, setHistory,
 * setPrompt or touch the live transcript ref; its navigation/query/cursor
 * state belongs solely to the secondary pane.
 */
function AnsfHistoryPane({activeChatId,chats,onClose}){
  const [browseChatId,setBrowseChatId]=useState(activeChatId||'');
  const [entries,setEntries]=useState([]);
  const [before,setBefore]=useState(null);
  const [hasMore,setHasMore]=useState(false);
  const [hasNewer,setHasNewer]=useState(false);
  const [query,setQuery]=useState('');
  const [matches,setMatches]=useState([]);
  const [reading,setReading]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [assetProject,setAssetProject]=useState('lu');
  const [assetName,setAssetName]=useState('');
  const [assetVersion,setAssetVersion]=useState('');
  const [assetSource,setAssetSource]=useState('');
  const [assetRows,setAssetRows]=useState([]);
  const [assetBusy,setAssetBusy]=useState(false);
  const [semanticModelId,setSemanticModelId]=useState('');
  const [semanticBusy,setSemanticBusy]=useState(false);
  const [semanticReceipt,setSemanticReceipt]=useState('');
  const [resumeProject,setResumeProject]=useState('lu');
  const [resumeTask,setResumeTask]=useState('windows-installer');
  const [resumeSource,setResumeSource]=useState('');
  const [resumeCompleted,setResumeCompleted]=useState('');
  const [resumeNext,setResumeNext]=useState('');
  const [resumeBlockers,setResumeBlockers]=useState('');
  const [resumeRecord,setResumeRecord]=useState(null);
  const [contextChange,setContextChange]=useState(null);
  const [resumeBusy,setResumeBusy]=useState(false);
  const [effectCallId,setEffectCallId]=useState('');
  const [verifiedEffects,setVerifiedEffects]=useState([]);

  const requests=useRef(0);
  const scrollRef=useRef(null);
  const pendingPaneAnchor=useRef(null);
  useEffect(()=>{
    if(!activeChatId)return;
    invoke('lu_m3_embedding_status',{chatId:activeChatId})
      .then(model=>setSemanticModelId(model||''))
      .catch(()=>setSemanticModelId(''));
  },[activeChatId]);
  useEffect(()=>{
    if(!activeChatId||!browseChatId)return;
    const request=++requests.current;
    setEntries([]);setBefore(null);setHasMore(false);setHasNewer(false);
    setReading(null);setError('');setBusy(true);
    invoke('lu_m3_history_page',{
      chatId:activeChatId,sourceChatId:browseChatId,
      beforeEntryId:null,limit:24
    }).then(page=>{
      if(request!==requests.current)return;
      setEntries(page.entries);setBefore(page.nextBeforeId);
      setHasMore(page.hasMore);
    }).catch(e=>{
      if(request===requests.current)setError('Older messages unavailable: '+String(e));
    }).finally(()=>{if(request===requests.current)setBusy(false)});
    return ()=>{requests.current+=1;};
  },[activeChatId,browseChatId]);
  const PANE_VISIBLE_ROWS=192;
  function originalEntryNumber(hit){
    const id=Number(hit?.sourceId?.slice(hit.sourceId.lastIndexOf(':')+1));
    if(!Number.isSafeInteger(id)||id<1)throw Error('Invalid canonical ANSF source sequence');
    return id;
  }
  function mergeOriginalPages(existing,incoming,direction){
    const byId=new Map();
    for(const entry of [...existing,...incoming]){
      originalEntryNumber(entry);
      byId.set(entry.sourceId,entry);
    }
    const sorted=[...byId.values()].sort((x,y)=>originalEntryNumber(x)-originalEntryNumber(y));
    const evicted=sorted.length>PANE_VISIBLE_ROWS;
    return {rows:direction==='older'
      ?sorted.slice(0,PANE_VISIBLE_ROWS):sorted.slice(-PANE_VISIBLE_ROWS),evicted};
  }
  async function older(){
    if(!hasMore||!before||busy||!activeChatId)return;
    const request=++requests.current;setBusy(true);setError('');
    pendingPaneAnchor.current=captureHistoryAnchor(scrollRef.current);
    try{
      const page=await invoke('lu_m3_history_page',{
        chatId:activeChatId,sourceChatId:browseChatId,
        beforeEntryId:before,limit:24
      });
      if(request!==requests.current)return;
      const merged=mergeOriginalPages(entries,page.entries,'older');
      setEntries(merged.rows);
      setBefore(page.nextBeforeId);setHasMore(page.hasMore);
      if(merged.evicted)setHasNewer(true);
    }catch(e){
      pendingPaneAnchor.current=null;
      if(request===requests.current)setError('Older page unavailable: '+String(e));
    }finally{if(request===requests.current)setBusy(false)}
  }
  async function newer(){
    if(!hasNewer||busy||!activeChatId||!entries.length)return;
    const request=++requests.current;setBusy(true);setError('');
    pendingPaneAnchor.current=captureHistoryAnchor(scrollRef.current);
    const sourceId=originalEntryNumber(entries.at(-1));
    try{
      const page=await invoke('lu_m3_history_page_after',{
        chatId:activeChatId,sourceChatId:browseChatId,
        afterEntryId:sourceId,limit:24
      });
      if(request!==requests.current)return;
      const merged=mergeOriginalPages(entries,page.entries,'newer');
      setEntries(merged.rows);
      setHasNewer(page.hasMore);
      if(merged.evicted){
        setBefore(originalEntryNumber(merged.rows[0]));
        setHasMore(true);
      }
    }catch(e){
      pendingPaneAnchor.current=null;
      if(request===requests.current)setError('Newer page unavailable: '+String(e));
    }finally{if(request===requests.current)setBusy(false)}
  }
  function onHistoryScroll(){
    const el=scrollRef.current;
    if(!el||busy)return;
    if(el.scrollTop<140&&hasMore)void older();
    else if(el.scrollHeight-el.scrollTop-el.clientHeight<140&&hasNewer)void newer();
  }
  useLayoutEffect(()=>{
    const el=scrollRef.current;
    if(el&&pendingPaneAnchor.current!==null){
      restoreHistoryAnchor(el,pendingPaneAnchor.current);
      pendingPaneAnchor.current=null;
    }
  },[entries]);
  async function search(e){
    e.preventDefault();
    if(!activeChatId||!query.trim()||busy)return;
    const request=++requests.current;setBusy(true);setError('');
    try{
      if(query.trim().startsWith('ansf:v1:')){
        await openSource(query.trim());
        return;
      }
      const hits=await invoke('lu_m3_semantic_recall',{
        chatId:activeChatId,query:query.trim()
      });
      if(request===requests.current){setMatches(hits);setReading(null)}
    }catch(e){if(request===requests.current)setError('Search unavailable: '+String(e))}
    finally{if(request===requests.current)setBusy(false)}
  }
  async function openSource(sourceId){
    if(!activeChatId)return;
    const data=await invoke('lu_m3_source_text_page',{
      chatId:activeChatId,sourceId,offsetChars:0,limitChars:4096
    });
    setReading(data);
  }
  async function readMore(){
    if(!reading?.nextOffsetChars||busy||!activeChatId)return;
    const previous=reading;setBusy(true);setError('');
    try{
      const page=await invoke('lu_m3_source_text_page',{
        chatId:activeChatId,sourceId:previous.sourceId,
        offsetChars:previous.nextOffsetChars,limitChars:4096
      });
      setReading(current=>current?.sourceId===page.sourceId&&
        current?.nextOffsetChars===page.offsetChars?
        {...page,text:current.text+page.text}:current);
    }catch(e){setError('Source continuation unavailable: '+String(e))}
    finally{setBusy(false)}
  }
  async function openChecked(sourceId){
    if(busy)return;
    setBusy(true);setError('');
    try{await openSource(sourceId)}
    catch(e){setError('Original source unavailable: '+String(e))}
    finally{setBusy(false)}
  }
  async function enableSemantic(){
    if(!activeChatId||semanticBusy)return;
    setSemanticBusy(true);setError('');setSemanticReceipt('');
    try{
      const id=await invoke('lu_m3_embedding_enable',{chatId:activeChatId});
      if(id){setSemanticModelId(id);setSemanticReceipt(
        'Verified local embedding GGUF loaded; choose Index this book to add semantic candidates.'
      )}
    }catch(e){setError('Local semantic model could not start: '+String(e))}
    finally{setSemanticBusy(false)}
  }
  async function indexSemantic(){
    if(!activeChatId||!browseChatId||!semanticModelId||semanticBusy)return;
    setSemanticBusy(true);setError('');
    try{
      const result=await invoke('lu_m3_semantic_index',{
        chatId:activeChatId,sourceChatId:browseChatId,limit:12
      });
      setSemanticReceipt(
        result.indexedNow+' new original entries indexed, '+result.totalIndexed+
        ' total for this book. Repeat to index more.'
      );
    }catch(e){setError('Semantic source indexing stopped: '+String(e))}
    finally{setSemanticBusy(false)}
  }
  async function disableSemantic(){
    if(!activeChatId||semanticBusy)return;
    setSemanticBusy(true);setError('');
    try{
      await invoke('lu_m3_embedding_disable',{chatId:activeChatId});
      setSemanticModelId('');setSemanticReceipt(
        'Local embedding worker stopped. Original ANSF and rebuildable vector index preserved.'
      );
    }catch(e){setError('Local embedding model shutdown failed: '+String(e))}
    finally{setSemanticBusy(false)}
  }
  async function inspectAssets(){
    if(!activeChatId||!assetProject.trim()||assetBusy)return;
    setAssetBusy(true);setError('');
    try{
      const rows=await invoke('lu_m3_operational_inventory',{
        chatId:activeChatId,projectId:assetProject.trim()
      });
      setAssetRows(rows);
    }catch(e){setError('Build asset inventory unavailable: '+String(e))}
    finally{setAssetBusy(false)}
  }
  async function rememberAsset(e){
    e.preventDefault();
    const sourceId=assetSource.trim()||reading?.sourceId;
    if(!activeChatId||!assetProject.trim()||!assetName.trim()||
       !assetVersion.trim()||!sourceId||assetBusy)return;
    setAssetBusy(true);setError('');
    try{
      const result=await invoke('lu_m3_remember_asset',{
        chatId:activeChatId,projectId:assetProject.trim(),
        logicalName:assetName.trim(),version:assetVersion.trim(),
        sourceId
      });
      if(result){
        setAssetRows(items=>[result,...items.filter(r=>r.recordId!==result.recordId)]);
        setAssetSource(sourceId);
      }
    }catch(e){setError('Local package was not recorded: '+String(e))}
    finally{setAssetBusy(false)}
  }
  async function resumeCheckpoint(){
    if(!activeChatId||!resumeProject.trim()||!resumeTask.trim()||resumeBusy)return;
    setResumeBusy(true);setError('');
    try{
      const value=await invoke('lu_m3_resume_continuity',{
        chatId:activeChatId,projectId:resumeProject.trim(),
        taskId:resumeTask.trim()
      });
      setResumeRecord(value);
      setContextChange(null);
      setVerifiedEffects([]);
      // Read-only automatic reconciliation: re-hash registered packages and
      // recheck APPLIED journal receipts before presenting a next-step plan.
      if(value){
        const [assets,effects,changes]=await Promise.all([
          invoke('lu_m3_resume_resources',{
            chatId:activeChatId,projectId:resumeProject.trim()
          }),
          invoke('lu_m3_task_effects',{
            chatId:activeChatId,projectId:resumeProject.trim(),
            taskId:resumeTask.trim()
          }),
          invoke('lu_m3_context_change',{
            chatId:activeChatId,projectId:resumeProject.trim(),
            taskId:resumeTask.trim()
          })
        ]);
        setContextChange(changes);
        setAssetProject(resumeProject.trim());
        setAssetRows(assets);
        setVerifiedEffects(effects);
      }
    }catch(e){setError('Task continuity lookup unavailable: '+String(e))}
    finally{setResumeBusy(false)}
  }
  async function saveCheckpoint(e){
    e.preventDefault();
    const sourceId=resumeSource.trim()||
      (reading?.sourceChatId===activeChatId?reading.sourceId:null);
    if(!activeChatId||!sourceId||!resumeCompleted.trim()||
       !resumeNext.trim()||resumeBusy)return;
    setResumeBusy(true);setError('');
    try{
      const saved=await invoke('lu_m3_record_continuity',{
        chatId:activeChatId,projectId:resumeProject.trim(),
        taskId:resumeTask.trim(),sourceIds:[sourceId],
        completedWork:resumeCompleted.trim(),nextAction:resumeNext.trim(),
        blockers:resumeBlockers.split('\n').map(x=>x.trim()).filter(Boolean).slice(0,32)
      });
      if(saved)setResumeRecord(saved);
    }catch(e){setError('Checkpoint not saved: '+String(e))}
    finally{setResumeBusy(false)}
  }
  async function linkAppliedEffect(e){
    e.preventDefault();
    if(!activeChatId||!resumeRecord||!effectCallId.trim()||resumeBusy)return;
    setResumeBusy(true);setError('');
    try{
      const proof=await invoke('lu_m3_link_applied_effect',{
        chatId:activeChatId,projectId:resumeRecord.projectId,
        taskId:resumeRecord.taskId,callId:effectCallId.trim()
      });
      if(proof){
        setVerifiedEffects(old=>[...old.filter(x=>x.callId!==proof.callId),proof]);
        setEffectCallId('');
      }
    }catch(e){setError('Effect receipt was not verified: '+String(e))}
    finally{setResumeBusy(false)}
  }
  return <aside className="ansfHistoryPane" aria-label="Independent ANSF history browser">
    <header className="ansfHistoryHeader">
      <div><strong>ANSF Library</strong><small>Independent history · active chat unchanged</small></div>
      <button type="button" className="subtleButton" onClick={onClose}
        aria-label="Close history browser">Close</button>
    </header>
    <label className="ansfHistoryPicker">Browse original conversation
      <select value={browseChatId} onChange={e=>setBrowseChatId(e.target.value)}
        aria-label="Browse history of conversation">
        {chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title}</option>)}
      </select>
    </label>
    <form onSubmit={search} className="ansfHistorySearch">
      <input type="search" value={query} maxLength={256}
        onChange={e=>setQuery(e.target.value)}
        placeholder="Keywords or exact ansf:v1: ID"
        aria-label="Search historical messages"/>
      <button type="submit" disabled={busy||!query.trim()}>Search</button>
    </form>
    {error&&<p role="alert" className="ansfHistoryError">{error}</p>}
    <small>Historical source text is read-only context, never a new tool permission.</small>
    <details className="ansfAssetsMemory" aria-label="Local semantic retrieval">
      <summary>Local semantic search · original-source hybrid</summary>
      <small>Optional, offline-only GGUF embedding worker. No cloud history transfer; the SQLite originals remain authoritative. Search falls back to lexical recall when unloaded.</small>
      <p>{semanticModelId
        ?'Local semantic model loaded: '+semanticModelId
        :'No separate local embedding model is loaded.'}</p>
      {semanticReceipt&&<small>{semanticReceipt}</small>}
      {!semanticModelId?<button type="button" disabled={semanticBusy}
        onClick={()=>void enableSemantic()}>
          {semanticBusy?'Starting local engine…':'Choose local embedding GGUF…'}
        </button>:<div>
        <button type="button" disabled={semanticBusy}
          onClick={()=>void indexSemantic()}>
          {semanticBusy?'Indexing originals…':'Index next 12 entries of this book'}
        </button>
        <button type="button" disabled={semanticBusy}
          onClick={()=>void disableSemantic()}>Stop embedding worker</button>
      </div>}
      <small>New original messages are indexed incrementally. An unindexed or unsuitable embedding model does not grant answers or permissions.</small>
    </details>

    <details className="ansfAssetsMemory" aria-label="Project continuity">
      <summary>Continue previous work · project checkpoint</summary>
      <small>Source-linked task progress remains available across Lu conversations and model changes. Historical work does not grant a new tool operation.</small>
      <form className="ansfAssetForm" onSubmit={saveCheckpoint}>
        <label>Project ID
          <input value={resumeProject} maxLength={128}
            onChange={e=>setResumeProject(e.target.value)}/>
        </label>
        <label>Task ID
          <input value={resumeTask} maxLength={128}
            onChange={e=>setResumeTask(e.target.value)}/>
        </label>
        <label>ANSF evidence from this conversation
          <input value={resumeSource||
             (reading?.sourceChatId===activeChatId?reading?.sourceId:'')}
            maxLength={224} onChange={e=>setResumeSource(e.target.value)}
            placeholder="Source ID from this original chat"/>
        </label>
        <label>Work completed and verified
          <textarea value={resumeCompleted} maxLength={4096} rows={2}
            onChange={e=>setResumeCompleted(e.target.value)}/>
        </label>
        <label>Next action
          <textarea value={resumeNext} maxLength={4096} rows={2}
            onChange={e=>setResumeNext(e.target.value)}/>
        </label>
        <label>Unresolved blockers (one per line)
          <textarea value={resumeBlockers} maxLength={4096} rows={2}
            onChange={e=>setResumeBlockers(e.target.value)}/>
        </label>
        <button type="submit" disabled={resumeBusy||!resumeCompleted.trim()||
          !resumeNext.trim()||!(resumeSource||
          (reading?.sourceChatId===activeChatId&&reading?.sourceId))}>
          Confirm and save checkpoint…</button>
      </form>
      <button type="button" disabled={resumeBusy||!resumeProject.trim()||!resumeTask.trim()}
        onClick={()=>void resumeCheckpoint()}>
        {resumeBusy?'Reading checkpoint…':'Find latest task checkpoint'}</button>
      {resumeRecord&&<section className="ansfHistoryEntry" aria-label="Verified project continuity">
        <strong>{resumeRecord.projectId} · {resumeRecord.taskId}</strong>
        <small>Origin chat {resumeRecord.sourceChatId} · original evidence {resumeRecord.originalSourceIds.join(', ')}</small>
        {contextChange?.needsReview&&<section role="status" aria-label="Context Change Monitor review">
          <strong>Context Change Monitor · review changed task assumptions</strong>
          <small>Compared checkpoints {contextChange.previousCheckpointId} and {contextChange.currentCheckpointId}. Historical changes never approve a new action.</small>
          {contextChange.nextActionChanged&&<p>Next action changed. Review before continuing.</p>}
          {contextChange.completedWorkChanged&&<p>Completed-work description changed; verify the APPLIED receipts.</p>}
          {contextChange.sourceEvidenceChanged&&<p>Original ANSF source evidence changed; reconcile supersession.</p>}
          {contextChange.addedBlockers.map(b=><p key={'added-'+b}>New blocker: {b}</p>)}
          {contextChange.clearedBlockers.map(b=><p key={'cleared-'+b}>Cleared blocker (verify): {b}</p>)}
        </section>}
        <p>Completed: {resumeRecord.completedWork}</p>
        <p>Next: {resumeRecord.nextAction}</p>
        {resumeRecord.blockers.map((b,i)=><p key={i}>Blocker: {b}</p>)}
        <small>This is context, not current permission to run any tool.</small>
        <strong>Previously completed native effects (verified receipts)</strong>
        {verifiedEffects.length===0&&<small>No linked applied tool receipts.</small>}
        {verifiedEffects.map(e=><small key={e.callId}>
          {e.toolId} · {e.callId} · SHA-256 {e.receiptSha256}
        </small>)}
        <form className="ansfAssetForm" onSubmit={linkAppliedEffect}>
          <label>Previously completed tool call ID
            <input value={effectCallId} maxLength={128}
              onChange={e=>setEffectCallId(e.target.value)}
              placeholder="Native effect journal call ID"/>
          </label>
          <button type="submit" disabled={resumeBusy||!effectCallId.trim()}>
            Confirm and link existing APPLIED receipt…</button>
        </form>
        <strong>Automatically revalidated project resources</strong>
        <small>Only verified_present files may be considered for reuse. 
          A file identity never grants execution.</small>
        {assetRows.filter(a=>a.projectId===resumeRecord.projectId).map(asset=><small key={asset.recordId}>
          {asset.logicalName} {asset.version} · {asset.presence}
        </small>)}
      </section>}
    </details>
    <details className="ansfAssetsMemory">
      <summary>Operational memory · reusable build assets</summary>
      <small>Host-verified file hashes and source IDs; a remembered path is not an execution permission.</small>
      <form className="ansfAssetForm" onSubmit={rememberAsset}>
        <label>Project ID
          <input value={assetProject} maxLength={128}
            onChange={e=>setAssetProject(e.target.value)} placeholder="lu"/>
        </label>
        <label>Asset label
          <input value={assetName} maxLength={180}
            onChange={e=>setAssetName(e.target.value)} placeholder="Windows SDK ZIP"/>
        </label>
        <label>Version or build identity
          <input value={assetVersion} maxLength={180}
            onChange={e=>setAssetVersion(e.target.value)} placeholder="10.0.x / SHA build"/>
        </label>
        <label>Original ANSF source ID
          <input value={assetSource||reading?.sourceId||''} maxLength={224}
            onChange={e=>setAssetSource(e.target.value)}
            placeholder="First open an original source"/>
        </label>
        <button type="submit" disabled={assetBusy||!assetName.trim()||
          !assetVersion.trim()||!(assetSource||reading?.sourceId)}>
          Choose file and remember hash…</button>
      </form>
      <button type="button" disabled={assetBusy||!assetProject.trim()}
        onClick={()=>void inspectAssets()}>{assetBusy?'Verifying…':'Recheck saved assets'}</button>
      <div className="ansfAssetRows">
        {assetRows.map(asset=><article key={asset.recordId}>
          <strong>{asset.logicalName}</strong><small>{asset.version} · {asset.presence}</small>
          <code>{asset.location}</code>
          <small>{asset.bytes} bytes · SHA-256: {asset.sha256}</small>
        </article>)}
      </div>
    </details>
    <div className="ansfHistoryScroll" ref={scrollRef} onScroll={onHistoryScroll}>
      {hasMore&&<button type="button" onClick={()=>void older()} disabled={busy}>
        {busy?'Loading…':'Load older page'}</button>}
      {entries.map(entry=><article className="ansfHistoryEntry" key={entry.sourceId}
        data-ansf-seq={originalEntryNumber(entry)}>
        <small>{entry.role==='user'?'You':'Lu'} · {entry.chatTitle}</small>
        <p>{entry.excerpt}{entry.truncated?' …':''}</p>
        <button type="button" className="subtleButton" disabled={busy}
          onClick={()=>void openChecked(entry.sourceId)}>Read full original</button>
        <code>{entry.sourceId}</code>
      </article>)}
      {hasNewer&&<button type="button" onClick={()=>void newer()} disabled={busy}>
        {busy?"Loading…":"Load newer original page"}</button>}
      {!entries.length&&!busy&&<small>No completed original entries on this page.</small>}
      {matches.length>0&&<section aria-label="Historical search results">
        <h3>Matching originals</h3>
        {matches.map(hit=><article className="ansfHistoryEntry" key={hit.sourceId}>
          <strong>{hit.chatTitle}</strong><p>{hit.excerpt}</p>
          <button type="button" className="subtleButton" disabled={busy}
            onClick={()=>void openChecked(hit.sourceId)}>Read matching original</button>
        </article>)}
      </section>}
    </div>
    {reading&&<section className="ansfOriginalReader" aria-label="Exact original source reader">
      <header><strong>Original · {reading.role}</strong>
        <button type="button" onClick={()=>setReading(null)}>Close source</button></header>
      <pre>{reading.text}</pre>
      <small>{reading.text.length} displayed characters of {reading.totalChars} · {reading.sourceId}</small>
      {reading.nextOffsetChars!==null&&<button type="button" disabled={busy}
        onClick={()=>void readMore()}>Read next source page</button>}
    </section>}
  </aside>;
}

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [modelSearch, setModelSearch] = useState('');
  const [providerQuery,setProviderQuery] = useState('');
  const [providerHits,setProviderHits] = useState([]);
  const [providerReview,setProviderReview] = useState(null);
  const [providerTransferIntent,setProviderTransferIntent] = useState(null);
  const [providerBusy,setProviderBusy] = useState(false);
  const [providerFailure,setProviderFailure] = useState('');
  const [modelInspection, setModelInspection] = useState(null);
  const [modelBackends,setModelBackends] = useState([]);
  const [runtime, setRuntime] = useState({ phase: 'Idle', modelRef: null, sessionId: null });
  const [chats, setChats] = useState([]);
  const [folders, setFolders] = useState([]);
  const [chatId, setChatId] = useState(null);
  // M2 host-authored goal/clarification context, separate from normal chat.
  const [m2Context,setM2Context]=useState(null);
  const [m2GoalDraft,setM2GoalDraft]=useState('');
  const [m2QuestionDraft,setM2QuestionDraft]=useState('');
  const [m2Comments,setM2Comments]=useState({});
  const [m2PanelOpen,setM2PanelOpen]=useState(false);
  const [m2Busy,setM2Busy]=useState(false);

  const [historyPaneOpen,setHistoryPaneOpen]=useState(false);
  const [history, setHistory] = useState([]);
  // Active transcript is a bounded visible window, not the entire ANSF book.
  const [historyWindow,setHistoryWindow]=useState({
    older:null,hasOlder:false,hasNewer:false
  });
  const loadingWindow=useRef(false);
  const pendingPrepend=useRef(null);

  const [prompt, setPrompt] = useState('');
  const [images, setImages] = useState([]);
  const [captureLocalTaskId,setCaptureLocalTaskId]=useState(null);
  const [imageViewer,setImageViewer] = useState(null);
  const [imageEdit,setImageEdit]=useState({mode:'resize',x:'0',y:'0',width:'',height:'',blockSize:'12',stroke:'3',color:'#ff5500',amount:'0',sigma:'2',threshold:'3'});
  const [imagePlan,setImagePlan]=useState({chatId:null,imageRef:null,steps:[],redo:[]});
  const [imagePreviewBusy,setImagePreviewBusy]=useState(false);
  const [imageAssemblyPreview,setImageAssemblyPreview]=useState(null);
  const imagePreviewKeyRef=useRef('');
  const [imageExportBusy,setImageExportBusy]=useState(false);
  const [imageSelection,setImageSelection]=useState(null);
  const imageDragRef=useRef(null);
  const [stageSelection,setStageSelection]=useState(null);
  const stageDragRef=useRef(null);
  const [imageExportReceipt,setImageExportReceipt]=useState(null);
  const [imageHistoryView,setImageHistoryView]=useState({chatId:null,rows:[]});
  const [imageHistoryBusy,setImageHistoryBusy]=useState(false);
  const [ocrBusy,setOcrBusy]=useState(false);
  const [ocrObservation,setOcrObservation]=useState(null);
  const [ocrReviewText,setOcrReviewText]=useState('');
  const [ocrRegion,setOcrRegion]=useState({x:'0',y:'0',width:'',height:''});
  const [ocrPdfPage,setOcrPdfPage]=useState('1');
  const [ocrPdfEndPage,setOcrPdfEndPage]=useState('1');
  const [ocrPdfBatch,setOcrPdfBatch]=useState([]);
  const [ocrPdfIndex,setOcrPdfIndex]=useState(0);
  const [ocrPdfEdits,setOcrPdfEdits]=useState({});
  const [readerSelector,setReaderSelector]=useState('');
  const [readerContinuation,setReaderContinuation]=useState(null);
  const [visionCapability,setVisionCapability] = useState(null);
  const [visionProbeBusy,setVisionProbeBusy]=useState(false); // session diagnostic only, no model attestation
  const [observeView,setObserveView] = useState({mode:'off'}); // Lu-only semantic hints
  const [observeNotes,setObserveNotes] = useState([]);
  const [observeHandoff,setObserveHandoff] = useState(null);
  const [captureWorkerReady,setCaptureWorkerReady]=useState(false);
  const [captureTaskId,setCaptureTaskId]=useState(null);
  const [captureReceipt,setCaptureReceipt]=useState(null);
  const [captureStarting,setCaptureStarting]=useState(false);
  const [browserVisibility,setBrowserVisibility] = useState('automatic_hidden');
  const [browserPreferenceError,setBrowserPreferenceError] = useState('');
  const [browserRuntime,setBrowserRuntime] = useState(null);
  const [browserOrigins,setBrowserOrigins]=useState('https://example.org');
  const [browserBusy,setBrowserBusy]=useState(false);
  const [browserReadBusy,setBrowserReadBusy]=useState(false);
  const [browserReadResult,setBrowserReadResult]=useState(null);
  const [browserReadError,setBrowserReadError]=useState('');
  const [researchAvailable,setResearchAvailable]=useState(false);
  const [researchQuery,setResearchQuery]=useState('');
  const [researchBusy,setResearchBusy]=useState(false);
  const [researchResult,setResearchResult]=useState(null);
  const [researchMetadataByChat,setResearchMetadataByChat]=useState({});
  const [researchCitationStyle,setResearchCitationStyle]=useState('apa7');
  const [researchCitationBusy,setResearchCitationBusy]=useState(false);
  const [researchBibliographyBusy,setResearchBibliographyBusy]=useState(false);
  const [researchBibliographiesByChat,setResearchBibliographiesByChat]=useState({});
  const [researchBibliographyExportsByChat,setResearchBibliographyExportsByChat]=useState({});
  const [sblScriptureBook,setSblScriptureBook]=useState('Genesis');
  const [sblScriptureTranslation,setSblScriptureTranslation]=useState('NRSV');
  const [sblScriptureChapter,setSblScriptureChapter]=useState('1');
  const [sblScriptureFirstVerse,setSblScriptureFirstVerse]=useState('1');
  const [sblScriptureLastVerse,setSblScriptureLastVerse]=useState('');
  const [sblScripturePreview,setSblScripturePreview]=useState(null);
  const [sblEditionId,setSblEditionId]=useState('NA28');
  const [sblEditionPreview,setSblEditionPreview]=useState(null);


  const [researchCitationsByChat,setResearchCitationsByChat]=useState({});
  const [researchCitationRevisionByChat,setResearchCitationRevisionByChat]=useState({});

  const [researchDocumentsByChat,setResearchDocumentsByChat]=useState({});
  const [researchDocumentActive,setResearchDocumentActive]=useState(null);
  const [researchCampaignsByChat,setResearchCampaignsByChat]=useState({});
  const [researchCampaignReviewByChat,setResearchCampaignReviewByChat]=useState({});
  const [researchCampaignRowsByChat,setResearchCampaignRowsByChat]=useState({});
  const [researchCampaignCeiling,setResearchCampaignCeiling]=useState(120);
  const [researchCampaignBusy,setResearchCampaignBusy]=useState(false);
  const [researchClaimDraft,setResearchClaimDraft]=useState({
    sourceId:'',claim:'',quote:'',stance:'COMPLICATES',reasoning:''
  });
  const [researchClaimsByChat,setResearchClaimsByChat]=useState({});
  const [researchReportTitle,setResearchReportTitle]=useState('Research evidence report');
  const [researchReportIncludeBibliography,setResearchReportIncludeBibliography]=useState(false);
  const [wordReplaceFormat,setWordReplaceFormat]=useState('docx');
  const [wordReplaceOld,setWordReplaceOld]=useState('');
  const [wordReplaceNew,setWordReplaceNew]=useState('');
  const [wordReplaceBusy,setWordReplaceBusy]=useState(false);
  const [wordReplaceReceipt,setWordReplaceReceipt]=useState(null);

  const [researchReportSavedByChat,setResearchReportSavedByChat]=useState({});
  const [researchReportBusy,setResearchReportBusy]=useState(false);
  const [researchDocumentQuotes,setResearchDocumentQuotes]=useState({});
  const [researchDocumentPassages,setResearchDocumentPassages]=useState({});
  const [researchError,setResearchError]=useState('');
  const [browserEvidenceByChat,setBrowserEvidenceByChat]=useState({});
  const [browserEvidenceBusy,setBrowserEvidenceBusy]=useState(false);
  const [researchQuoteDraftByKey,setResearchQuoteDraftByKey]=useState({});
  const [researchQuoteSourceByKey,setResearchQuoteSourceByKey]=useState({});
  const [researchQuoteProofByKey,setResearchQuoteProofByKey]=useState({});
  const [researchQuoteBusy,setResearchQuoteBusy]=useState(false);
  const [textFiles, setTextFiles] = useState([]);
  const [jobs, setJobs] = useState({}); // bounded, per-chat host generation state
  const [readProposals, setReadProposals] = useState({}); // completed model proposals, never grants
  const [writeProposals, setWriteProposals] = useState({}); // inert completed model write previews
  const [writeRecovery, setWriteRecovery] = useState([]); // read-only durable evidence
  const [editProposals, setEditProposals] = useState({});
  const [nativeActionProposals,setNativeActionProposals]=useState({}); // inert Wave 3 review intents
  const [toolActivityOpen,setToolActivityOpen]=useState(false);
  const [toolHistoryByChat,setToolHistoryByChat]=useState({});
  const [toolProgressByChat,setToolProgressByChat]=useState({});
  const [turnCapabilitiesByChat,setTurnCapabilitiesByChat]=useState({});
  const [toolHistoryBusy,setToolHistoryBusy]=useState(false);
  const [editReviewed, setEditReviewed] = useState({}); // UI only; native dialog grants authority
  const [editRecovery, setEditRecovery] = useState([]);
  const [editObservations, setEditObservations] = useState({});
  const [rollbackReviewed, setRollbackReviewed] = useState({});
  const [rollbackHistory, setRollbackHistory] = useState([]);
  const [latestRestored, setLatestRestored] = useState(null);
  const [latestEdited, setLatestEdited] = useState(null);
  const [backgroundPaneOpen,setBackgroundPaneOpen] = useState(false);
  const [backgroundTasks,setBackgroundTasks] = useState([]);
  const [backgroundGuardian,setBackgroundGuardian] = useState(null);
  const [backgroundDelay,setBackgroundDelay] = useState('0');
  const [backgroundRepeat,setBackgroundRepeat] = useState('0');
  const [backgroundPriority,setBackgroundPriority] = useState('normal');
  const [backgroundError,setBackgroundError] = useState('');
  const [backgroundFilter,setBackgroundFilter] = useState('active');
  const [backgroundTerminalNotice,setBackgroundTerminalNotice] = useState(null);
  const [autoBackgroundContinuation,setAutoBackgroundContinuation] = useState(false);
  const [working, setWorking] = useState(false);
  // An opt-in for exactly one ordinary Send. This boolean is never authority:
  // the host still requires an actual native directory picker.
  const [workspaceToolsNextSend,setWorkspaceToolsNextSend] = useState(false);
  // Explicit one-turn Browser opt-in; native dialog remains the grant.
  const [browserResearchNextSend,setBrowserResearchNextSend]=useState(false);
  const [modelLoadSeconds, setModelLoadSeconds] = useState(0);
  const [modelLoadTrace, setModelLoadTrace] = useState(null);
  const [lastHostLoadReply, setLastHostLoadReply] = useState(0);
  const [error, setError] = useState('');
  const [activity, setActivity] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [advancedSettingsOpen, setAdvancedSettingsOpen] = useState(false);
  const [documentSettings, setDocumentSettings] = useState(null);
  const [latestCreated, setLatestCreated] = useState(null);
  const [archiveBatchReview,setArchiveBatchReview]=useState(null);
  const [archiveBatchReviewed,setArchiveBatchReviewed]=useState(false);
  const [archiveBatchBusy,setArchiveBatchBusy]=useState(false);
  const [archiveBatchCancelRequested,setArchiveBatchCancelRequested]=useState(false);
  const [latestArchiveBatch,setLatestArchiveBatch]=useState(null);
  const [archiveReview, setArchiveReview] = useState(null);
  const [archiveReviewed, setArchiveReviewed] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const [archiveCancelRequested, setArchiveCancelRequested] = useState(false);
  const [latestArchiveExtract, setLatestArchiveExtract] = useState(null);
  const [p08PartMib,setP08PartMib]=useState(4);
  const [p08Busy,setP08Busy]=useState(false);
  const [p08ResultByChat,setP08ResultByChat]=useState({});

  const [documentComposerOpen, setDocumentComposerOpen] = useState(false);
  const [documentDraft, setDocumentDraft] = useState({format:'docx',title:'',content:''});
  const [documentCreateReview, setDocumentCreateReview] = useState(null);
  const [documentCreateReviewed, setDocumentCreateReviewed] = useState(false);
  const [documentCreateBusy, setDocumentCreateBusy] = useState(false);
  const [latestDocumentCreated, setLatestDocumentCreated] = useState(null);
  const [officeRevisionOpen, setOfficeRevisionOpen] = useState(false);
  const [officePatch, setOfficePatch] = useState({
    kind:'word_text',textIndex:0,slideNumber:1,sheetNumber:1,
    address:'A1',expectedOld:'',newValue:''
  });
  const [officeRevisionReview, setOfficeRevisionReview] = useState(null);
  const [officeRevisionReviewed, setOfficeRevisionReviewed] = useState(false);
  const [officeRevisionBusy, setOfficeRevisionBusy] = useState(false);
  const [latestOfficeRevision, setLatestOfficeRevision] = useState(null);
  const [officeBatchOpen, setOfficeBatchOpen] = useState(false);
  const [officeBatchDraft, setOfficeBatchDraft] = useState([
    {kind:'word_text',textIndex:0,slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''},
    {kind:'word_text',textIndex:1,slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''}
  ]);
  const [officeBatchReview, setOfficeBatchReview] = useState(null);
  const [officeBatchReviewed, setOfficeBatchReviewed] = useState(false);
  const [officeBatchBusy, setOfficeBatchBusy] = useState(false);
  const [latestOfficeBatch, setLatestOfficeBatch] = useState(null);
  const [fileTransferReview, setFileTransferReview] = useState(null);
  const [fileTransferReviewed, setFileTransferReviewed] = useState(false);
  const [fileTransferBusy, setFileTransferBusy] = useState(false);
  const [latestFileTransfer, setLatestFileTransfer] = useState(null);
  const [trashReview, setTrashReview] = useState(null);
  const [trashReviewed, setTrashReviewed] = useState(false);
  const [restoreReview, setRestoreReview] = useState(null);
  const [restoreReviewed, setRestoreReviewed] = useState(false);
  const [trashBusy, setTrashBusy] = useState(false);
  const [trashHistoryOpen, setTrashHistoryOpen] = useState(false);
  const [trashHistory, setTrashHistory] = useState([]);
  const [latestCustodyOutcome, setLatestCustodyOutcome] = useState(null);
  const [wave2RecoveryByChat,setWave2RecoveryByChat] = useState({});
  const [wave2RecoveryBusy,setWave2RecoveryBusy] = useState(false);
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
  const viewedDraftImage=imageViewer && images.find(im=>im.imageRef===imageViewer.imageRef);
  const loaded = runtime.phase === 'Ready' || runtime.phase === 'Generating';
  const busy = working || runtime.phase === 'Generating' || Object.keys(jobs).length > 0;
  const wave2FileBusy = archiveBusy || documentCreateBusy || officeRevisionBusy ||
    fileTransferBusy || trashBusy;
  const activeChat = chats.find(c => c.chatId === chatId);
  const matchingChats = chats.filter(c => matchesChatTitle(c, chatQuery));
  const visibleJob = chatId ? jobs[chatId] : null;
  const visibleStream = visibleJob?.text ?? '';
  const chatBusy = Boolean(visibleJob);
  const activeBackgroundTasks=backgroundTasks.filter(task=>
    ['PENDING','QUEUED','RUNNING','PAUSED'].includes(task.state));
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
  useEffect(() => {
    if (!(working && activity.startsWith('Loading'))) {
      setModelLoadTrace(null);
      setLastHostLoadReply(0);
      return;
    }
    let mounted = true;
    let pending = false;
    async function sampleHost() {
      if (pending) return; // One outstanding probe; no unbounded request buildup.
      pending = true;
      try {
        const trace = await invoke('lu_load_diagnostics');
        if (mounted) {
          setModelLoadTrace(trace);
          setLastHostLoadReply(Date.now());
        }
      } catch {
        // A failed probe is not proof of a GPU or WebView deadlock.
        if (mounted) setLastHostLoadReply(0);
      } finally {
        pending = false;
      }
    }
    sampleHost();
    const timer = setInterval(sampleHost, 1500);
    return () => { mounted = false; clearInterval(timer); };
  }, [working, activity]);
  useEffect(() => () => { for (const url of previewUrls.current) URL.revokeObjectURL(url); }, []);
  useEffect(()=>{
    if(!settingsOpen)return;
    let active=true;
    invoke('lu_browser_runtime_status')
      .then(v=>{if(active)setBrowserRuntime(v);})
      .catch(e=>{if(active)setBrowserPreferenceError(String(e));});
    return()=>{active=false;};
  },[settingsOpen]);
  useEffect(()=>{
    if(!settingsOpen)return;
    invoke('lu_research_available')
      .then(v=>setResearchAvailable(Boolean(v)))
      .catch(()=>setResearchAvailable(false));
  },[settingsOpen]);
  async function runResearchDiscovery(page=1){
    const selectedPage=Number.isInteger(page)?page:1;
    if(!chatId||researchBusy||!researchAvailable
      ||selectedPage<1||selectedPage>5)return;
    const sourceChat=chatId;
    setResearchBusy(true);
    setResearchError('');
    setResearchResult(null);
    try{
      const result=await invoke('lu_research_openalex',{chatId:sourceChat,query:researchQuery,page:selectedPage});
      // Preserve the requesting chat as part of the displayed evidence scope.
      setResearchResult({...result,chatId:sourceChat});
    }catch(e){setResearchError(String(e));}
    finally{setResearchBusy(false);}
  }
  async function previewSblCriticalEdition(){
    if(!chatId)return;
    setSblEditionPreview(null);setResearchError('');
    try{
      const receipt=await invoke('lu_research_sbl_critical_edition_locator',{
        chatId,editionId:sblEditionId,book:sblScriptureBook,
        chapter:Number(sblScriptureChapter),
        firstVerse:Number(sblScriptureFirstVerse),
        lastVerse:sblScriptureLastVerse.trim()?Number(sblScriptureLastVerse):null
      });
      setSblEditionPreview(receipt);
    }catch(e){setResearchError(String(e));}
  }
  async function previewSblScriptureLocator(){
    if(!chatId)return;
    setSblScripturePreview(null);setResearchError('');
    try{
      const receipt=await invoke('lu_research_sbl_scripture_locator',{
        chatId,book:sblScriptureBook,
        chapter:Number(sblScriptureChapter),
        firstVerse:Number(sblScriptureFirstVerse),
        lastVerse:sblScriptureLastVerse.trim()?Number(sblScriptureLastVerse):null,
        translation:sblScriptureTranslation
      });
      setSblScripturePreview(receipt);
    }catch(e){setResearchError(String(e));}
  }
  async function previewResearchBibliography(){
    if(!chatId||researchBibliographyBusy)return;
    const sourceChat=chatId;
    setResearchBibliographyBusy(true);setResearchError('');
    setResearchBibliographiesByChat(old=>({...old,[sourceChat]:null}));
    try{
      const view=await invoke('lu_research_bibliography_preview',{
        chatId:sourceChat,style:researchCitationStyle
      });
      setResearchBibliographiesByChat(old=>({...old,[sourceChat]:view}));
    }catch(e){setResearchError(String(e));}
    finally{setResearchBibliographyBusy(false);}
  }
  async function exportResearchBibliography(format){
    if(!chatId||researchBibliographyBusy)return;
    const sourceChat=chatId;
    setResearchBibliographyBusy(true);setResearchError('');
    try{
      const receipt=await invoke('lu_research_bibliography_export',{
        chatId:sourceChat,style:researchCitationStyle,format
      });
      if(receipt)setResearchBibliographyExportsByChat(old=>({
        ...old,[sourceChat]:receipt
      }));
    }catch(e){setResearchError(String(e));}
    finally{setResearchBibliographyBusy(false);}
  }
  async function reviewResearchCitationRevision(sourceId){
    if(!chatId||!researchAvailable||researchBusy||researchCitationBusy||!sourceId)return;
    const sourceChat=chatId;
    const key=sourceChat+'|'+sourceId;
    setResearchCitationBusy(true);setResearchError('');
    try{
      const receipt=await invoke('lu_research_citation_review_revision',{
        chatId:sourceChat,sourceId
      });
      if(receipt){
        setResearchCitationRevisionByChat(old=>({...old,[key]:receipt}));
        setResearchCitationsByChat(old=>({...old,[key]:null}));
        setResearchBibliographiesByChat(old=>({...old,[sourceChat]:null}));
        setResearchBibliographyExportsByChat(old=>({...old,[sourceChat]:null}));
      }
    }catch(e){setResearchError(String(e));}
    finally{setResearchCitationBusy(false);}
  }
  async function previewResearchCitation(sourceId){
    if(!chatId||!researchAvailable||researchBusy||researchCitationBusy||!sourceId)return;
    const sourceChat=chatId;
    const key=sourceChat+'|'+sourceId;
    setResearchCitationBusy(true);setResearchError('');
    // An older citation preview cannot survive a newly reviewed style.
    setResearchCitationsByChat(old=>({...old,[key]:null}));
    try{
      const receipt=await invoke('lu_research_citation_preview',{
        chatId:sourceChat,sourceId,style:researchCitationStyle
      });
      setResearchCitationsByChat(old=>({...old,[key]:receipt}));
      setResearchBibliographiesByChat(old=>({...old,[sourceChat]:null}));
    }catch(e){setResearchError(String(e));}
    finally{setResearchCitationBusy(false);}
  }
  async function checkResearchMetadata(sourceId){
    if(!chatId||!researchAvailable||researchBusy||!sourceId)return;
    const sourceChat=chatId;
    setResearchBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_crossref',{chatId:sourceChat,sourceId});
      setResearchMetadataByChat(old=>({
        ...old,[sourceChat+'|'+sourceId]:result
      }));
    }catch(e){setResearchError(String(e));}
    finally{setResearchBusy(false);}
  }
  async function refreshResearchClaims(selectedChat=chatId){
    if(!selectedChat)return;
    try{
      const claims=await invoke('lu_research_claims',{chatId:selectedChat});
      setResearchClaimsByChat(old=>({...old,[selectedChat]:claims}));
    }catch(error){setResearchError(String(error));}
  }
  async function submitResearchClaim(){
    if(!chatId||researchReportBusy)return;
    const sourceChat=chatId;
    const {sourceId,claim,quote,stance,reasoning}=researchClaimDraft;
    if(!sourceId.trim()||!claim.trim()||!quote.trim()||!reasoning.trim())return;
    setResearchReportBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_add_claim',{
        chatId:sourceChat,sourceId:sourceId.trim(),claim:claim.trim(),
        quote:quote.trim(),stance,reasoning:reasoning.trim()
      });
      setResearchClaimsByChat(old=>({...old,[sourceChat]:result}));
    }catch(error){setResearchError(String(error));}
    finally{setResearchReportBusy(false);}
  }
  async function exportResearchReport(format){
    if(!chatId||researchReportBusy||!researchReportTitle.trim())return;
    const sourceChat=chatId;
    setResearchReportBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_export_report',{
        chatId:sourceChat,title:researchReportTitle.trim(),format,
        includeBibliography:researchReportIncludeBibliography,
        citationStyle:researchCitationStyle
      });
      if(result)setResearchReportSavedByChat(old=>({...old,[sourceChat]:result}));
    }catch(error){setResearchError(String(error));}
    finally{setResearchReportBusy(false);}
  }
  async function refreshResearchCampaigns(selectedChat=chatId){
    if(!selectedChat)return;
    try{
      const result=await invoke('lu_research_campaigns',{chatId:selectedChat});
      setResearchCampaignsByChat(old=>({...old,[selectedChat]:result}));
    }catch(error){setResearchError(String(error));}
  }
  async function runResearchCampaign(){
    if(!chatId||researchCampaignBusy||!researchAvailable||!researchQuery.trim())return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_campaign_start',{
        chatId:selectedChat,query:researchQuery.trim(),
        candidateCeiling:researchCampaignCeiling
      });
      setResearchCampaignsByChat(old=>({
        ...old,[selectedChat]:[result,...(old[selectedChat]||[])
          .filter(existing=>existing.campaignId!==result.campaignId)]
      }));
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function reviewResearchCampaign(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const sourceChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      const receipt=await invoke('lu_research_campaign_review',{
        chatId:sourceChat,campaignId,limit:10
      });
      setResearchCampaignReviewByChat(old=>({
        ...old,[sourceChat+'|'+campaignId]:receipt
      }));
    }catch(error){setResearchError(String(error));}
    finally{setResearchCampaignBusy(false);}
  }
  async function showResearchReviewRows(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    try{
      const rows=await invoke('lu_research_campaign_review_rows',{
        chatId:selectedChat,campaignId
      });
      setResearchCampaignRowsByChat(old=>({
        ...old,[selectedChat+'|'+campaignId]:rows
      }));
    }catch(error){setResearchError(String(error));}
  }
  async function retryResearchSource(campaignId,sourceId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      await invoke('lu_research_campaign_retry_source',{
        chatId:selectedChat,campaignId,sourceId
      });
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await showResearchReviewRowsForChat(selectedChat,campaignId);
    }
  }
  async function showResearchReviewRowsForChat(selectedChat,campaignId){
    try{
      const rows=await invoke('lu_research_campaign_review_rows',{
        chatId:selectedChat,campaignId
      });
      setResearchCampaignRowsByChat(old=>({
        ...old,[selectedChat+'|'+campaignId]:rows
      }));
    }catch(error){setResearchError(String(error));}
  }
  async function resumeResearchCampaign(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      await invoke('lu_research_campaign_resume',{chatId:selectedChat,campaignId});
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function cancelResearchCampaign(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      await invoke('lu_research_campaign_cancel',{chatId:selectedChat,campaignId});
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function retrieveResearchDocument(sourceId){
    if(!chatId||researchBusy||!researchAvailable)return;
    const selectedChat=chatId;
    setResearchBusy(true);setResearchError('');
    setResearchDocumentActive({chatId:selectedChat,sourceId});
    try{
      const document=await invoke('lu_research_retrieve_document',{
        chatId:selectedChat,sourceId
      });
      setResearchDocumentsByChat(old=>({
        ...old,[selectedChat+'|'+sourceId]:document
      }));
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchDocumentActive(null);setResearchBusy(false);
    }
  }
  async function cancelResearchDocument(){
    const active=researchDocumentActive;
    if(!active||!chatId||active.chatId!==chatId)return;
    try{
      const requested=await invoke('lu_research_cancel_document',{
        chatId:active.chatId,sourceId:active.sourceId
      });
      if(requested){
        setResearchError('Document retrieval cancellation requested; native worker will be reaped before task completion.');
      }
    }catch(error){setResearchError(String(error));}
  }
  async function verifyResearchDocumentPassage(sourceId){
    if(!chatId||researchBusy)return;
    const selectedChat=chatId;
    const key=selectedChat+'|'+sourceId;
    const quote=(researchDocumentQuotes[key]||'').trim();
    if(quote.length<4||quote.length>4096)return;
    setResearchBusy(true);setResearchError('');
    try{
      const proof=await invoke('lu_research_verify_document_passage',{
        chatId:selectedChat,sourceId,quote
      });
      setResearchDocumentPassages(old=>({...old,[key]:proof}));
    }catch(error){
      setResearchDocumentPassages(old=>({...old,[key]:null}));
      setResearchError(String(error));
    }finally{setResearchBusy(false);}
  }
  async function reviewBrowserEvidence(){
    if(!chatId||browserEvidenceBusy)return;
    const selectedChat=chatId;
    setBrowserEvidenceBusy(true);setResearchError('');
    try{
      const list=await invoke('lu_research_browser_evidence',{chatId:selectedChat});
      // Keep the host's read-only records scoped to the selected conversation.
      setBrowserEvidenceByChat(old=>({...old,[selectedChat]:list}));
    }catch(e){setResearchError(String(e));}
    finally{setBrowserEvidenceBusy(false);}
  }
  async function verifyObservedQuote(item){
    if(!chatId||researchQuoteBusy)return;
    const sourceChat=chatId;
    const key=sourceChat+'|'+item.generationId+'|'+item.callId;
    const quote=(researchQuoteDraftByKey[key]||'').trim();
    if(quote.length<4||quote.length>2048)return;
    setResearchQuoteBusy(true);setResearchError('');
    try{
      const publisherId=(researchQuoteSourceByKey[key]||'').trim();
      const result=await invoke('lu_research_verify_quote',{
        chatId:sourceChat,generationId:item.generationId,callId:item.callId,
        quote,publisherSourceId:publisherId||null
      });
      setResearchQuoteProofByKey(old=>({...old,[key]:result}));
    }catch(e){
      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
      setResearchError(String(e));
    }finally{setResearchQuoteBusy(false);}
  }
  async function inspectWithBrowserModel(){
    if(!chatId||!runtime.sessionId||!browserRuntime?.running||browserReadBusy)return;
    const selectedChat=chatId;
    setBrowserReadBusy(true);setBrowserReadError('');setBrowserReadResult(null);
    try{
      const result=await invoke('lu_browser_model_read',{
        sessionId:runtime.sessionId,chatId:selectedChat
      });
      setBrowserReadResult(result);
      setBrowserRuntime(await invoke('lu_browser_runtime_status'));
    }catch(error){
      setBrowserReadError(String(error));
      await invoke('lu_browser_runtime_status')
        .then(setBrowserRuntime).catch(()=>{});
    }finally{setBrowserReadBusy(false);}
  }
  async function browserOperation(name,args={}){
    if(browserBusy)return;
    setBrowserBusy(true);
    try{
      const result=await invoke(name,args);
      if(name==='lu_browser_stop')setBrowserRuntime(await invoke('lu_browser_runtime_status'));
      else setBrowserRuntime(result);
      setBrowserPreferenceError('');
    }catch(e){setBrowserPreferenceError(String(e));}
    finally{setBrowserBusy(false);}
  }
  useEffect(()=>{
    invoke('lu_observe_capture_available')
      .then(v=>setCaptureWorkerReady(Boolean(v)))
      .catch(()=>setCaptureWorkerReady(false));
  },[]);
  useEffect(()=>{
    setCaptureTaskId(null);
    if(!chatId){
      // Telling the native host that there is no active chat revokes any
      // Lu-UI observation lease rather than letting it run unseen.
      invoke('lu_observe_status',{chatId:''}).catch(()=>{});
      setObserveView({mode:'off'});return;
    }
    let active=true;
    invoke('lu_observe_status',{chatId})
      .then(v=>{if(active)setObserveView(v);})
      .catch(()=>{if(active)setObserveView({mode:'off'});});
    // An OS-owned clock/watchdog may expire consent while Lu is idle.
    const timer=setInterval(()=>{
      invoke('lu_observe_status',{chatId})
        .then(v=>{if(active)setObserveView(v);}).catch(()=>{});
    },5000);
    return()=>{active=false;clearInterval(timer);};
  },[chatId]);
  useEffect(()=>{
    if(!chatId||observeView.chatId!==chatId||observeView.mode==='off'){
      setObserveNotes([]);setObserveHandoff(null);return;
    }
    let active=true;
    invoke('lu_observe_notes',{chatId})
      .then(notes=>{if(active)setObserveNotes(notes);})
      .catch(()=>{if(active)setObserveNotes([]);});
    return()=>{active=false;};
  },[chatId,observeView.sessionId,observeView.mode,observeView.notesLogged]);
  useEffect(()=>{
    // Report only enum hints from Lu itself. No text, pixels, clipboard,
    // keystrokes, external app content, or native test PASS/FAIL.
    if(!chatId||observeView.mode!=='active'||observeView.chatId!==chatId)return;
    invoke('lu_observe_self_signal',{chatId,kind:'ui_changed'})
      .then(v=>setObserveView(v))
      .catch(()=>{});
  },[chatId,runtime.phase,observeView.mode]);
  useEffect(()=>{
    if(!chatId||!error||observeView.mode!=='active'||observeView.chatId!==chatId)return;
    invoke('lu_observe_self_signal',{chatId,kind:'error_indicator'})
      .then(v=>setObserveView(v)).catch(()=>{});
  },[chatId,error,observeView.mode]);
  async function loadImageRevisionHistory(){
    if(!chatId||imageHistoryBusy)return;
    const sourceChat=chatId;
    setImageHistoryBusy(true);
    try{
      const rows=await invoke('lu_image_workspace_history',{chatId:sourceChat});
      setImageHistoryView({chatId:sourceChat,rows:Array.isArray(rows)?rows:[]});
    }catch(e){setError(String(e));}
    finally{setImageHistoryBusy(false);}
  }
  async function replaceWordTextRuns(){
    if(!chatId||wordReplaceBusy||!wordReplaceOld.trim()||!wordReplaceNew.trim()
       ||wordReplaceOld===wordReplaceNew)return;
    setWordReplaceBusy(true);setWordReplaceReceipt(null);setError('');
    try{
      const receipt=await invoke(
        wordReplaceFormat==='pptx'
          ?'lu_office_presentation_replace_all':'lu_office_word_replace_all',{
          chatId,oldText:wordReplaceOld,newText:wordReplaceNew
        }
      );
      if(receipt)setWordReplaceReceipt(receipt);
    }catch(e){setError(String(e));}
    finally{setWordReplaceBusy(false);}
  }
  async function verifyNativeVision(){
    if(!chatId||!loaded||visionProbeBusy)return;
    setVisionProbeBusy(true);setVisionCapability(null);setError('');
    try{
      const result=await invoke('lu_vision_run_probe',{chatId});
      setVisionCapability(result);
    }catch(e){setError(String(e));}
    finally{setVisionProbeBusy(false);}
  }
  const ocrImageScopeRef=useRef('');
  ocrImageScopeRef.current=JSON.stringify([chatId,imageViewer?.imageRef||null]);
  async function readAttachedImageText(){
    const sourceChat=chatId,sourceRef=imageViewer?.imageRef;
    if(!sourceChat||!sourceRef||ocrBusy)return;
    const key=JSON.stringify([sourceChat,sourceRef]);
    setOcrBusy(true);setOcrObservation(null);setOcrReviewText('');setError('');
    try{
      const result=await invoke('lu_ocr_read_attached_image',{
        chatId:sourceChat,imageRef:sourceRef
      });
      if(result&&ocrImageScopeRef.current===key){
        setOcrObservation({chatId:sourceChat,imageRef:sourceRef,...result});
        setOcrReviewText(result.text.slice(0,12_000));
      }
    }catch(e){if(ocrImageScopeRef.current===key)setError(String(e));}
    finally{setOcrBusy(false);}
  }
  async function readAttachedRegionText(){
    const sourceChat=chatId,sourceRef=imageViewer?.imageRef;
    if(!sourceChat||!sourceRef||ocrBusy)return;
    const key=JSON.stringify([sourceChat,sourceRef]);
    const x=Number(ocrRegion.x),y=Number(ocrRegion.y);
    const width=ocrRegion.width===''?imageViewer.width-x:Number(ocrRegion.width);
    const height=ocrRegion.height===''?imageViewer.height-y:Number(ocrRegion.height);
    if([x,y,width,height].some(v=>!Number.isSafeInteger(v))||
       x<0||y<0||width<=0||height<=0||
       x>=imageViewer.width||y>=imageViewer.height||
       width>imageViewer.width-x||height>imageViewer.height-y){
      setError('OCR rectangle must fit within the original image pixels.');return;
    }
    setOcrBusy(true);setOcrObservation(null);setOcrReviewText('');setError('');
    try{
      const result=await invoke('lu_ocr_read_attached_region',{
        chatId:sourceChat,imageRef:sourceRef,x,y,width,height
      });
      if(result&&ocrImageScopeRef.current===key){
        setOcrObservation({chatId:sourceChat,imageRef:sourceRef,...result});
        setOcrReviewText(result.text.slice(0,12_000));
      }
    }catch(e){if(ocrImageScopeRef.current===key)setError(String(e));}
    finally{setOcrBusy(false);}
  }
  async function readSelectedPdfPage(){
    if(!chatId||ocrBusy)return;
    const page=Number(ocrPdfPage),pageEnd=Number(ocrPdfEndPage);
    if(!Number.isSafeInteger(page)||!Number.isSafeInteger(pageEnd)||
       page<1||pageEnd<page||pageEnd>32||pageEnd-page>=8){
      setError('Choose 1–8 consecutive PDF pages between 1 and 32.');return;
    }
    const sourceChat=chatId,key=ocrImageScopeRef.current;
    setOcrBusy(true);setOcrObservation(null);setOcrReviewText('');
    setOcrPdfBatch([]);setOcrPdfEdits({});setOcrPdfIndex(0);setError('');
    try{
      const result=await invoke('lu_ocr_read_selected_pdf_page',{
        chatId:sourceChat,page,pageEnd
      });
      if(Array.isArray(result)&&result.length&&ocrImageScopeRef.current===key){
        const pages=result.map(value=>({...value,chatId:sourceChat}));
        setOcrPdfBatch(pages);setOcrObservation(pages[0]);
        setOcrReviewText(pages[0].text.slice(0,12_000));
      }
    }catch(e){if(ocrImageScopeRef.current===key)setError(String(e));}
    finally{setOcrBusy(false);}
  }
  function selectOcrPdfPage(index){
    const result=ocrPdfBatch[index];
    if(!result||result.chatId!==chatId)return;
    setOcrPdfEdits(previous=>({...previous,
      [ocrObservation?.page]:ocrReviewText}));
    setOcrPdfIndex(index);setOcrObservation(result);
    setOcrReviewText(ocrPdfEdits[result.page]??result.text.slice(0,12_000));
  }
  async function cancelOfflineOcr(){
    if(!chatId||!ocrBusy)return;
    try{await invoke('lu_ocr_cancel',{chatId});}
    catch(e){setError(String(e));}
  }
  async function readOfflineImageText(){
    if(!chatId||ocrBusy)return;
    const sourceChat=chatId,key=ocrImageScopeRef.current;
    setOcrBusy(true);setOcrObservation(null);setOcrReviewText('');setError('');
    try{
      const result=await invoke('lu_ocr_read_selected_image',{chatId:sourceChat});
      if(result&&ocrImageScopeRef.current===key){
        setOcrObservation({chatId:sourceChat,...result});
        setOcrReviewText(result.text.slice(0,12_000));
      }
    }catch(e){if(ocrImageScopeRef.current===key)setError(String(e));}
    finally{setOcrBusy(false);}
  }
  // User-only pointer selection; the trusted native host revalidates all
  // image regions before cropping, OCR, preview or effect publication.
  function selectImagePointerStart(event){
    if(event.button!==0||!imageViewer?.width||!imageViewer?.height)return;
    const origin=imagePointerPixel(
      event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),
      imageViewer.width,imageViewer.height
    );
    if(!origin)return;
    imageDragRef.current={chatId,imageRef:imageViewer.imageRef,origin};
    setImageSelection({chatId,imageRef:imageViewer.imageRef,
      x:origin.x,y:origin.y,width:1,height:1});
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }
  function selectImagePointerMove(event){
    const drag=imageDragRef.current;
    if(!drag||drag.chatId!==chatId||drag.imageRef!==imageViewer?.imageRef)return;
    const to=imagePointerPixel(
      event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),
      imageViewer.width,imageViewer.height
    );
    const rectangle=imageDragRectangle(drag.origin,to,
      imageViewer.width,imageViewer.height);
    if(rectangle)setImageSelection({chatId,imageRef:drag.imageRef,...rectangle});
  }
  function selectImagePointerEnd(event){
    const drag=imageDragRef.current;
    imageDragRef.current=null;
    if(!drag||drag.chatId!==chatId||drag.imageRef!==imageViewer?.imageRef)return;
    const to=imagePointerPixel(
      event.clientX,event.clientY,event.currentTarget.getBoundingClientRect(),
      imageViewer.width,imageViewer.height
    );
    const region=imageDragRectangle(drag.origin,to,
      imageViewer.width,imageViewer.height);
    if(!region)return;
    setImageSelection({chatId,imageRef:drag.imageRef,...region});
    setOcrRegion(Object.fromEntries(
      Object.entries(region).map(([key,value])=>[key,String(value)])));
    const hasStagedEdits=imagePlan.chatId===chatId&&
      imagePlan.imageRef===imageViewer.imageRef&&imagePlan.steps.length>0;
    if(!hasStagedEdits){
      setImageEdit(v=>({...v,x:String(region.x),y:String(region.y),
        width:String(region.width),height:String(region.height)}));
    }
  }
  const currentImagePreviewKey=JSON.stringify([
    chatId,imageViewer?.imageRef||null,
    imagePlan.chatId===chatId&&imagePlan.imageRef===imageViewer?.imageRef
      ?imagePlan.steps:[]
  ]);
  imagePreviewKeyRef.current=currentImagePreviewKey;
  useEffect(()=>{
    setImageAssemblyPreview(null);
    setStageSelection(null);
    stageDragRef.current=null;
  },[currentImagePreviewKey]);
  function startStagedSelection(event){
    if(event.button!==0||imagePreviewBusy||
      imageAssemblyPreview?.key!==currentImagePreviewKey)return;
    const view=imageAssemblyPreview.result;
    const pixel=imagePointerPixel(event.clientX,event.clientY,
      event.currentTarget.getBoundingClientRect(),view.width,view.height);
    if(!pixel)return;
    stageDragRef.current={
      key:currentImagePreviewKey,origin:pixel,
      width:view.width,height:view.height
    };
    setStageSelection({key:currentImagePreviewKey,
      x:pixel.x,y:pixel.y,width:1,height:1});
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }
  function moveStagedSelection(event){
    const drag=stageDragRef.current;
    if(!drag||drag.key!==currentImagePreviewKey)return;
    const pixel=imagePointerPixel(event.clientX,event.clientY,
      event.currentTarget.getBoundingClientRect(),drag.width,drag.height);
    const rect=imageDragRectangle(drag.origin,pixel,drag.width,drag.height);
    if(rect)setStageSelection({key:drag.key,...rect});
  }
  function endStagedSelection(event){
    const drag=stageDragRef.current;stageDragRef.current=null;
    if(!drag||drag.key!==currentImagePreviewKey)return;
    const pixel=imagePointerPixel(event.clientX,event.clientY,
      event.currentTarget.getBoundingClientRect(),drag.width,drag.height);
    const rect=imageDragRectangle(drag.origin,pixel,drag.width,drag.height);
    if(!rect)return;
    setStageSelection({key:drag.key,...rect});
    setImageEdit(v=>({...v,x:String(rect.x),y:String(rect.y),
      width:String(rect.width),height:String(rect.height)}));
  }
  function queueImageStep(){
    if(!chatId||!imageViewer?.imageRef||imageExportBusy)return;
    const previous=imagePlan.chatId===chatId&&imagePlan.imageRef===imageViewer.imageRef
      ?imagePlan.steps:[];
    if(previous.length>=12){
      setError('A single image assembly permits at most 12 steps.');return;
    }
    const frame=stagedImageDimensions(imageViewer.width,imageViewer.height,previous);
    if(!frame){setError('Staged image geometry is invalid.');return;}
    const region=REGION_IMAGE_OPERATIONS.includes(imageEdit.mode);
    const geometryNeutral=['rotate90','flip_h','grayscale','invert','blur',
      'brightness','contrast','sharpen'].includes(imageEdit.mode);
    const width=geometryNeutral?frame.width:Number(imageEdit.width);
    const height=geometryNeutral?frame.height:Number(imageEdit.height);
    const x=region?Number(imageEdit.x):null;
    const y=region?Number(imageEdit.y):null;
    const isDrawing=['rectangle','line','ellipse','arrow'].includes(imageEdit.mode);
    const blockSize=imageEdit.mode==='pixelate'?Number(imageEdit.blockSize):null;
    const stroke=isDrawing?Number(imageEdit.stroke):null;
    const color=isDrawing&&/^#[0-9a-fA-F]{6}$/.test(imageEdit.color)
      ?parseInt(imageEdit.color.slice(1),16):null;
    const amount=['brightness','contrast'].includes(imageEdit.mode)
      ?Number(imageEdit.amount):null;
    const sigma=['blur','sharpen'].includes(imageEdit.mode)
      ?Number(imageEdit.sigma):null;
    const threshold=imageEdit.mode==='sharpen'?Number(imageEdit.threshold):null;
    const step={operation:imageEdit.mode,x,y,width,height,blockSize,stroke,color,
      amount,sigma,threshold};
    const next=[...previous,step];
    const after=stagedImageDimensions(imageViewer.width,imageViewer.height,next);
    if(!after){
      setError('Image operation exceeds current staged dimensions or limits.');return;
    }
    setImagePlan({chatId,imageRef:imageViewer.imageRef,steps:next,redo:[]});
    setImageEdit(v=>({...v,x:'0',y:'0',
      width:String(after.width),height:String(after.height)}));
  }
  function undoImageStep(){
    if(!chatId||imagePlan.chatId!==chatId||
      imagePlan.imageRef!==imageViewer?.imageRef||!imagePlan.steps.length)return;
    const steps=imagePlan.steps.slice(0,-1);
    const dims=stagedImageDimensions(imageViewer.width,imageViewer.height,steps);
    if(!dims)return;
    setImagePlan({...imagePlan,steps,redo:[imagePlan.steps.at(-1),...imagePlan.redo]});
    setImageEdit(v=>({...v,width:String(dims.width),height:String(dims.height)}));
  }
  function redoImageStep(){
    if(!chatId||imagePlan.chatId!==chatId||
      imagePlan.imageRef!==imageViewer?.imageRef||!imagePlan.redo.length)return;
    const steps=[...imagePlan.steps,imagePlan.redo[0]];
    const dims=stagedImageDimensions(imageViewer.width,imageViewer.height,steps);
    if(!dims){setError('Redo would exceed image geometry limits.');return;}
    setImagePlan({...imagePlan,steps,redo:imagePlan.redo.slice(1)});
    setImageEdit(v=>({...v,width:String(dims.width),height:String(dims.height)}));
  }
  async function previewImageAssembly(){
    const sourceChat=chatId,sourceRef=imageViewer?.imageRef;
    if(!sourceChat||!sourceRef||imagePreviewBusy||
       imagePlan.chatId!==sourceChat||imagePlan.imageRef!==sourceRef||
       !imagePlan.steps.length)return;
    const steps=imagePlan.steps.map(step=>({...step}));
    const key=JSON.stringify([sourceChat,sourceRef,steps]);
    setImagePreviewBusy(true);
    try{
      const result=await invoke('lu_image_workspace_preview',{
        chatId:sourceChat,imageRef:sourceRef,steps
      });
      // A completed preview may not appear in another chat or over a
      // different assembly if the operator edited while native code ran.
      if(imagePreviewKeyRef.current===key){
        setImageAssemblyPreview({key,result});
      }
    }catch(e){if(imagePreviewKeyRef.current===key)setError(String(e));}
    finally{setImagePreviewBusy(false);}
  }
  async function exportImageRevision(){
    if(!chatId||!imageViewer?.imageRef||imageExportBusy)return;
    // A staged immutable assembly is authoritative for export. Values in
    // the currently visible single-step form must not veto a valid staged plan.
    const queuedPlan=imagePlan.chatId===chatId&&
      imagePlan.imageRef===imageViewer.imageRef&&imagePlan.steps.length?
      imagePlan.steps:null;
    const region=!queuedPlan&&REGION_IMAGE_OPERATIONS.includes(imageEdit.mode);
    const geometry=region||(!queuedPlan&&imageEdit.mode==='resize');
    const width=queuedPlan?1:(geometry?Number(imageEdit.width):imageViewer.width);
    const height=queuedPlan?1:(geometry?Number(imageEdit.height):imageViewer.height);
    const x=region?Number(imageEdit.x):0,y=region?Number(imageEdit.y):0;
    const blockSize=Number(imageEdit.blockSize);
    const stroke=Number(imageEdit.stroke);
    const hex=/^#[0-9a-fA-F]{6}$/.test(imageEdit.color||'')?
      parseInt(imageEdit.color.slice(1),16):null;
    const amount=Number(imageEdit.amount);
    const sigma=Number(imageEdit.sigma);
    const threshold=Number(imageEdit.threshold);
    const filterInvalid=(
      (imageEdit.mode==='brightness'&&(!Number.isSafeInteger(amount)||
        amount< -128||amount>128))||
      (imageEdit.mode==='contrast'&&(!Number.isSafeInteger(amount)||
        amount< -100||amount>100))||
      (['blur','sharpen'].includes(imageEdit.mode)&&
        (!Number.isSafeInteger(sigma)||sigma<1||sigma>8))||
      (imageEdit.mode==='sharpen'&&
        (!Number.isSafeInteger(threshold)||threshold<0||threshold>64))
    );
    if(!queuedPlan&&(!Number.isSafeInteger(width)||!Number.isSafeInteger(height)||
       width<1||height<1||width>8192||height>8192||width*height>32000000||
       (region&&(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||
         x<0||y<0||x+width>imageViewer.width||y+height>imageViewer.height))||
       filterInvalid||
       (imageEdit.mode==='pixelate'&&(!Number.isSafeInteger(blockSize)||
         blockSize<2||blockSize>128))||
       (['rectangle','line','ellipse','arrow'].includes(imageEdit.mode)&&
        (hex===null||!Number.isSafeInteger(stroke)||
         stroke<1||stroke>(['line','arrow'].includes(imageEdit.mode)?32:64)||
         (imageEdit.mode==='arrow'&&(width<3||height<3)))))){
      setError('Choose valid dimensions, mask/annotation region and color.');return;
    }
    setImageExportBusy(true);setImageExportReceipt(null);
    try{
      const saved=await invoke('lu_image_workspace_export',{
        chatId,imageRef:imageViewer.imageRef,
        // When steps are supplied, the native host ignores the single-step
        // operation. A validated harmless resize keeps the legacy signature.
        operation:queuedPlan?'resize':imageEdit.mode,
        x:region?x:null,y:region?y:null,width,height,
        blockSize:!queuedPlan&&imageEdit.mode==='pixelate'?blockSize:null,
        stroke:!queuedPlan&&['rectangle','line','ellipse','arrow'].includes(imageEdit.mode)?stroke:null,
        color:!queuedPlan&&['rectangle','line','ellipse','arrow'].includes(imageEdit.mode)?hex:null,
        amount:!queuedPlan&&['brightness','contrast'].includes(imageEdit.mode)?amount:null,
        sigma:!queuedPlan&&['blur','sharpen'].includes(imageEdit.mode)?sigma:null,
        threshold:!queuedPlan&&imageEdit.mode==='sharpen'?threshold:null,
        steps:queuedPlan
      });
      if(saved){
        setImageExportReceipt(saved);
        await loadImageRevisionHistory();
      }
    }catch(e){setError(String(e));}
    finally{setImageExportBusy(false);}
  }
  async function observeCaptureOneShot(foregroundOnly=false,primaryDisplay=false,localImage=false){
    if(!chatId||!captureWorkerReady||observeView.mode!=='active'||captureStarting)return;
    setCaptureStarting(true);
    try{
      const id=await invoke('lu_observe_capture_one_shot',{chatId,foregroundOnly,primaryDisplay,localImage});
      if(id){
        setCaptureTaskId(id);
        setCaptureLocalTaskId(localImage?id:null);
        setCaptureReceipt(null);
        setBackgroundPaneOpen(true);
      }
    }catch(e){setError(String(e));}
    finally{setCaptureStarting(false);}
  }
  async function attachLocalObservedFrame(){
    if(!chatId||!captureTaskId||captureLocalTaskId!==captureTaskId
       ||observeView.mode!=='active'||captureStarting)return;
    const activeChat=chatId;
    setCaptureStarting(true);
    try{
      const image=await invoke('lu_observe_capture_local_image',{chatId:activeChat});
      if(!image)throw new Error('No completed local frame available; inspect the native receipt first.');
      if(activeChatRef.current!==activeChat){
        await invoke('lu_release_image',{imageRef:image.imageRef});
        return;
      }
      setImages(prev=>prev.some(p=>p.imageRef===image.imageRef)?prev:[...prev,image]);
      setCaptureLocalTaskId(null);
      setActivity('Approved observed frame attached locally. Use the current multimodal GGUF to ask about its visible content.');
    }catch(e){setError(String(e));}
    finally{setCaptureStarting(false);}
  }
  async function reviewCaptureReceipt(){
    if(!chatId||!captureTaskId)return;
    try{
      const receipt=await invoke('lu_observe_capture_receipt',{chatId});
      setCaptureReceipt(receipt?{chatId,...receipt}:null);
    }catch(e){setError(String(e));}
  }
  async function forgetObserveNotes(){
    if(!chatId)return;
    const selectedChat=chatId;
    try{
      const removed=await invoke('lu_observe_forget_notes',{chatId:selectedChat});
      // null is a denied native confirmation, not a successful empty purge.
      if(removed===null||removed===undefined)return;
      if(selectedChat===chatId){
        setObserveNotes([]);
        setObserveHandoff(null);
        setObserveView(v=>({...v,notesLogged:0}));
        setActivity('Forgot '+removed+' local observation note'+(removed===1?'':'s')+'. No prior observation is reused.');
      }
    }catch(e){setError(String(e));}
  }
  async function reviewObserveHandoff(){
    if(!chatId||observeView.mode==='off')return;
    try{
      const handoff=await invoke('lu_observe_handoff',{chatId});
      setObserveHandoff(handoff);
    }catch(e){setError(String(e));}
  }
  function observeAction(action){
    if(!chatId)return;
    invoke('lu_observe_'+action,{chatId})
      .then(v=>{
        setObserveView(v);
        if(action==='pause'||action==='stop'){
          setCaptureTaskId(null);setCaptureReceipt(null);setCaptureLocalTaskId(null);
        }
        setError('');
      })
      .catch(e=>setError(String(e)));
  }
  useEffect(()=>{
    if(!settingsOpen)return;
    let active=true;
    invoke('lu_browser_visibility')
      .then(mode=>{if(active){setBrowserVisibility(mode);setBrowserPreferenceError('');}})
      .catch(e=>{if(active)setBrowserPreferenceError(String(e));});
    return()=>{active=false;};
  },[settingsOpen]);

  useEffect(()=>{
    if (!settingsOpen || !selectedModel) { setModelInspection(null); return; }
    let alive=true;
    setModelInspection(null);
    invoke('lu_model_backend_options').then(options=>{
      if(alive)setModelBackends(options);
    }).catch(()=>{if(alive)setModelBackends([]);});
    invoke('lu_model_preflight',{modelRef:selectedModel})
      .then(view=>{if(alive)setModelInspection(view);})
      .catch(()=>{if(alive)setModelInspection({
        modelRef:selectedModel,status:'inspection_unavailable',loadAllowed:false,
        detail:'Native model compatibility inspection failed. No model was loaded.'
      });});
    return()=>{alive=false;};
  },[settingsOpen,selectedModel,models]);

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
      const taskId=await invoke('lu_background_scan_file',{
        chatId:chatId||null,
        delayMinutes:Number(backgroundDelay),
        repeatMinutes:Number(backgroundRepeat),
        priority:backgroundPriority
      });
      if (taskId) {
        setBackgroundFilter('active');
        await refreshBackgroundTasks();
      }
    } catch (error) {
      setBackgroundError(String(error));
    }
  }
  useEffect(()=>{
    let alive=true;
    if (!chatId){
      setAutoBackgroundContinuation(false);
      return ()=>{alive=false;};
    }
    invoke('lu_background_auto_continue',{chatId,enabled:null})
      .then(value=>{if(alive)setAutoBackgroundContinuation(Boolean(value));})
      .catch(error=>{if(alive)setBackgroundError(String(error));});
    return ()=>{alive=false;};
  },[chatId]);
  async function changeAutoBackgroundContinuation(checked){
    if (!chatId)return;
    try{
      const saved=await invoke('lu_background_auto_continue',{
        chatId,enabled:Boolean(checked)
      });
      setAutoBackgroundContinuation(Boolean(saved));
      setBackgroundError('');
    }catch(error){
      setBackgroundError(String(error));
    }
  }
  async function cancelBackgroundTask(taskId) {
    try {
      await invoke('lu_background_cancel', {taskId,chatId});
      await refreshBackgroundTasks();
    } catch (error) {
      // Native dialog denial and races against completion are not success.
      await refreshBackgroundTasks();
      if (!String(error).includes('Cancellation declined')) {
        setBackgroundError(String(error));
      }
    }
  }
  async function reapproveScheduledTask(taskId){
    if(!chatId)return;
    try{
      const newId=await invoke('lu_background_reapprove',{chatId,reviewTaskId:taskId});
      if(newId)await refreshBackgroundTasks();
    }catch(error){
      setBackgroundError(String(error));
    }
  }
  async function changeQueuedBackgroundState(taskId,action){
    if(!chatId)return;
    try{
      await invoke('lu_background_queue_control',{chatId,taskId,action});
      await refreshBackgroundTasks();
    }catch(error){
      setBackgroundError(String(error));
    }
  }
  async function refreshBackgroundTasks() {
    try {
      if(!chatId){setBackgroundTasks([]);setBackgroundGuardian(null);return;}
      const [items,guardian]=await Promise.all([
        invoke('lu_background_tasks',{chatId}),
        invoke('lu_background_guardian_status',{chatId})
      ]);
      setBackgroundTasks(Array.isArray(items)?items:[]);
      setBackgroundGuardian(guardian||null);
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
  },[backgroundPaneOpen,chatId]);

  // M2 state is reloaded from the native ANSF+chat-journal reconciliation
  // for every conversation; no model event or localStorage confers authority.
  useEffect(()=>{
    let alive=true;
    setM2Context(null);
    setM2Comments({});
    setM2GoalDraft('');
    setM2QuestionDraft('');
    if(!chatId)return ()=>{alive=false;};
    invoke('lu_m2_goal_context',{chatId}).then(view=>{
      if(!alive||activeChatRef.current!==chatId)return;
      setM2Context({...view,chatId});
      setM2GoalDraft(view.goal||'');
    }).catch(error=>{if(alive&&activeChatRef.current===chatId)
      setError('Goal memory could not be reconciled: '+String(error));});
    return ()=>{alive=false;};
  },[chatId]);
  async function m2SetGoal(event){
    event.preventDefault();
    if(!chatId||m2Busy||!m2GoalDraft.trim())return;
    const target=chatId;setM2Busy(true);setError('');
    try{
      const view=await invoke('lu_m2_set_goal',{chatId:target,goal:m2GoalDraft});
      if(activeChatRef.current===target){
        setM2Context({...view,chatId:target});
        setM2GoalDraft(view.goal);
        setActivity('Goal saved in ANSF-backed memory');
      }
    }catch(error){setError('Goal not confirmed: '+String(error));}
    finally{setM2Busy(false);}
  }
  async function m2OpenQuestion(event){
    event.preventDefault();
    if(!chatId||m2Busy||!m2QuestionDraft.trim())return;
    const target=chatId;setM2Busy(true);setError('');
    try{
      const view=await invoke('lu_m2_open_question',{chatId:target,question:m2QuestionDraft});
      if(activeChatRef.current===target){
        setM2Context({...view,chatId:target});
        setM2QuestionDraft('');
      }
    }catch(error){setError('Question not recorded: '+String(error));}
    finally{setM2Busy(false);}
  }
  async function m2Answer(questionId,choice){
    if(!chatId||m2Busy)return;
    const target=chatId;setM2Busy(true);setError('');
    try{
      const comment=(m2Comments[questionId]||'').trim();
      const view=await invoke('lu_m2_answer_question',{
        chatId:target,questionId,choice,comment:comment||null
      });
      if(activeChatRef.current===target){
        setM2Context({...view,chatId:target});
        setM2Comments(old=>({...old,[questionId]:''}));
        setActivity(choice==='not_now'?'Clarification deferred; no answer or permission inferred'
          :'Clarification recorded for future turns');
      }
    }catch(error){setError('Clarification not recorded: '+String(error));}
    finally{setM2Busy(false);}
  }
  async function refreshChats() {
    const list = await invoke('lu_chats');
    setChats(list.chats);
    setFolders(list.folders);
    return list.chats;
  }
  const MAX_VISIBLE_CHAT_ROWS=MAX_HISTORY_ROWS;
  async function loadLatestUiHistory(id){
    const page=await invoke('lu_history_window',{
      chatId:id,cursor:null,direction:'older',limit:64
    });
    if(requestedChat.current===id||currentChat.current===id){
      setHistoryWindow({
        older:page.nextCursor,hasOlder:page.hasMore,hasNewer:false
      });
    }
    return page.entries;
  }
  // Reopen the original source near its remembered stable-ID cursor rather
  // than applying stale absolute pixel offsets to a different loaded window.
  async function loadRememberedUiHistory(id){
    const remembered=scrollByChat.current.get(id);
    const anchor=remembered?.anchor;
    if(!remembered?.follow&&Number.isSafeInteger(anchor?.seq)&&anchor.seq>0){
      const [page,newer]=await Promise.all([
        invoke('lu_history_window',{
          chatId:id,cursor:anchor.seq+1,direction:'older',limit:64
        }),
        invoke('lu_history_window',{
          chatId:id,cursor:anchor.seq,direction:'newer',limit:1
        })
      ]);
      if(requestedChat.current===id){
        setHistoryWindow({
          older:page.nextCursor,hasOlder:page.hasMore,
          hasNewer:newer.entries.length>0
        });
      }
      return page.entries;
    }
    return loadLatestUiHistory(id);
  }
  async function loadOlderUiHistory(){
    const id=currentChat.current;
    if(!id||loadingWindow.current||!historyWindow.hasOlder||
       historyWindow.older===null)return;
    loadingWindow.current=true;
    const anchor=captureHistoryAnchor(transcriptRef.current);
    try{
      const page=await invoke('lu_history_window',{
        chatId:id,cursor:historyWindow.older,direction:'older',limit:48
      });
      if(currentChat.current!==id)return;
      const merged=mergeHistoryWindow(history,page.entries,'older',MAX_VISIBLE_CHAT_ROWS);
      pendingPrepend.current={chatId:id,anchor};
      followOutput.current=false;
      setHistory(merged.rows);
      setHistoryWindow(prev=>({
        older:page.nextCursor,hasOlder:page.hasMore,
        hasNewer:prev.hasNewer||merged.evictedNewer
      }));
    }catch(e){setError('Previous history page unavailable: '+String(e))}
    finally{loadingWindow.current=false}
  }
  async function loadNewerUiHistory(){
    const id=currentChat.current;
    const last=history.at(-1)?.seq;
    if(!id||!last||loadingWindow.current||!historyWindow.hasNewer)return;
    loadingWindow.current=true;
    const anchor=captureHistoryAnchor(transcriptRef.current);
    try{
      const page=await invoke('lu_history_window',{
        chatId:id,cursor:last,direction:'newer',limit:48
      });
      if(currentChat.current!==id)return;
      const merged=mergeHistoryWindow(history,page.entries,'newer',MAX_VISIBLE_CHAT_ROWS);
      pendingPrepend.current={chatId:id,anchor};
      followOutput.current=false;
      setHistory(merged.rows);
      setHistoryWindow(prev=>({
        older:merged.evictedOlder?merged.rows[0]?.seq:prev.older,
        hasOlder:prev.hasOlder||merged.evictedOlder,
        hasNewer:page.hasMore
      }));
    }catch(e){setError('Next history page unavailable: '+String(e))}
    finally{loadingWindow.current=false}
  }
  async function switchChat(id) {
    if (currentChat.current !== id) {
      // Capture the outgoing reading position before the asynchronous read.
      const el = transcriptRef.current;
      if (el) scrollByChat.current.set(currentChat.current || 'new', {
        top: el.scrollTop, follow: followOutput.current,
        anchor: captureHistoryAnchor(el)
      });
      pendingDrafts.current.set(currentChat.current || 'new', {
        prompt: draftText.current, images: draftImages.current, files: draftFiles.current
      });
    }
    requestedChat.current = id;
    const entries = await loadRememberedUiHistory(id);
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
    let stopQueued = () => {}, stopStart = () => {}, stopToken = () => {}, stopReadProposal = () => {}, stopWriteProposal = () => {}, stopEditProposal = () => {}, stopNativeAction = () => {}, stopToolProgress = () => {}, stopTurnCapabilities = () => {}, stopBackground = () => {}, stopFinished = () => {}, stopBrowserChallenge = () => {};
    (async () => {
      try {
        const [list] = await Promise.all([refreshChats(), refreshCatalog()]);
        if (alive && list.length) await switchChat(list[0].chatId);
        if (alive) invoke('lu_document_settings')
          .then(value => {if (alive) setDocumentSettings(value);})
          .catch(error => {if (alive) setError(String(error));});
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
    listen('lu-browser-needs-human',e=>{
      if(!alive||!e.payload?.chatId)return;
      invoke('lu_browser_runtime_status')
        .then(v=>{if(alive)setBrowserRuntime(v);})
        .catch(()=>{});
    }).then(fn=>{if(alive)stopBrowserChallenge=fn;else fn();});
    listen('lu-token', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => tokenForJob(prev,e.payload));
    }).then(fn => { if (alive) stopToken = fn; else fn(); });
    listen('lu-read-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setReadProposals(prev => ({...prev, [proposal.proposalId]: proposal}));
    }).then(fn => { if (alive) stopReadProposal = fn; else fn(); });
    listen('lu-write-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setWriteProposals(prev => ({...prev, [proposal.proposalId]: proposal}));
    }).then(fn => { if (alive) stopWriteProposal = fn; else fn(); });
    listen('lu-edit-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setEditProposals(prev => ({...prev, [proposal.proposalId]:proposal}));
    }).then(fn => { if (alive) stopEditProposal = fn; else fn(); });
    listen('lu-native-action-proposal',e=>{
      const proposal=e.payload;
      if (!alive||!proposal?.chatId||!proposal?.proposalId)return;
      setNativeActionProposals(prev=>({...prev,[proposal.proposalId]:proposal}));
    }).then(fn=>{if(alive)stopNativeAction=fn;else fn();});
    listen('lu-native-tool-progress',e=>{
      const update=e.payload;
      if (!alive||!update?.chatId||!update?.generationId)return;
      if (!['model_step','tool_started','tool_returned'].includes(update.phase))return;
      setToolProgressByChat(prev=>({
        ...prev,[update.chatId]:update,
      }));
    }).then(fn=>{if(alive)stopToolProgress=fn;else fn();});
    listen('lu-native-turn-capabilities',e=>{
      const entry=e.payload;
      if (!alive||!entry?.chatId||!entry?.generationId||!Array.isArray(entry.tools))return;
      setTurnCapabilitiesByChat(prev=>({...prev,[entry.chatId]:entry}));
    }).then(fn=>{if(alive)stopTurnCapabilities=fn;else fn();});
    listen('lu-generation-finish', e=>{
      if (!alive || !e.payload?.chatId)return;
      const {chatId:finishedChat,generationId}=e.payload;
      setToolProgressByChat(prev=>{
        if (prev[finishedChat]?.generationId!==generationId)return prev;
        const next={...prev};delete next[finishedChat];return next;
      });
      setTurnCapabilitiesByChat(prev=>{
        if (prev[finishedChat]?.generationId!==generationId)return prev;
        const next={...prev};delete next[finishedChat];return next;
      });
      setJobs(prev=>prev[finishedChat]?.generationId===generationId
        ?removeJob(prev,finishedChat):prev);
      // Native auto turns aren't initiated by a WebView invoke. Read their
      // journaled assistant output, including after a background tab switch.
      if (currentChat.current===finishedChat){
        loadLatestUiHistory(finishedChat)
          .then(items=>{if(alive&&currentChat.current===finishedChat)setHistory(items);})
          .catch(()=>{});
      }
    }).then(fn=>{if(alive)stopFinished=fn;else fn();});
    listen('lu-background-terminal', e => {
      if (!alive || !e.payload?.taskId) return;
      // This event comes from the native host's persisted terminal ledger.
      // No AI generation or permission grant is performed by the WebView.
      setBackgroundTerminalNotice(e.payload);
      if (backgroundPaneOpen) void refreshBackgroundTasks();
    }).then(fn => { if (alive) stopBackground=fn; else fn(); });
    return () => { alive = false; stopQueued(); stopStart(); stopToken(); stopReadProposal(); stopWriteProposal(); stopEditProposal(); stopNativeAction(); stopToolProgress(); stopTurnCapabilities(); stopBackground(); stopFinished(); stopBrowserChallenge(); };
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
    if(el.scrollTop<140&&historyWindow.hasOlder)void loadOlderUiHistory();
    if(el.scrollHeight-el.scrollTop-el.clientHeight<140&&historyWindow.hasNewer)
      void loadNewerUiHistory();
  }
  function jumpToLatest() {
    const el = transcriptRef.current;
    if (!el) return;
    followOutput.current = true;
    el.scrollTop = el.scrollHeight;
    scrollByChat.current.set(currentChat.current || 'new', {top: el.scrollTop, follow:true});
    setAtBottom(true);
    if(historyWindow.hasNewer&&chatId){
      const target=chatId;
      void loadLatestUiHistory(target).then(rows=>{
        if(currentChat.current===target)setHistory(rows);
      }).catch(e=>setError('Latest history unavailable: '+String(e)));
    }
  }
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el) return;
    const stored = scrollByChat.current.get(chatId || 'new');
    followOutput.current = stored?.follow ?? true;
    if(stored?.anchor&&!stored.follow)restoreHistoryAnchor(el,stored.anchor);
    else el.scrollTop = restoredScrollTop(stored, el.scrollHeight, el.clientHeight);
    setAtBottom(followOutput.current);
  }, [chatId]);
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el || !followOutput.current || pendingPrepend.current) return;
    el.scrollTop = el.scrollHeight;
  }, [history, visibleStream, chatId]);
  useLayoutEffect(()=>{
    const el=transcriptRef.current;
    const pending=pendingPrepend.current;
    if(!el||!pending)return;
    if(pending.chatId===currentChat.current)restoreHistoryAnchor(el,pending.anchor);
    followOutput.current=false;
    pendingPrepend.current=null;
  },[history]);

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
  async function searchPublicModels(){
    if(providerBusy || !providerQuery.trim()) return;
    setProviderBusy(true);setProviderFailure('');setProviderHits([]);setProviderReview(null);setProviderTransferIntent(null);
    try{
      const hits=await invoke('lu_model_provider_search',{query:providerQuery.trim()});
      setProviderHits(hits);
    }catch(error){setProviderFailure(String(error));}
    finally{setProviderBusy(false);}
  }
  async function inspectPublicModel(lead){
    if(providerBusy)return;
    setProviderBusy(true);setProviderFailure('');setProviderReview(null);setProviderTransferIntent(null);
    try{
      const detail=await invoke('lu_model_provider_review',{
        repoId:lead.repoId,revision:lead.revision
      });
      if(detail.repoId!==lead.repoId||detail.revision!==lead.revision ||
          detail.downloadAvailable!==false || !detail.files?.length){
        throw Error('Native provider review returned an invalid or changed catalog result');
      }
      setProviderReview(detail);
    }catch(error){setProviderFailure(String(error));}
    finally{setProviderBusy(false);}
  }
  async function preparePinnedTransfer(file){
    if(providerBusy||!providerReview)return;
    setProviderBusy(true);setProviderFailure('');setProviderTransferIntent(null);
    try{
      const reviewed=await invoke('lu_model_prepare_transfer',{
        repoId:providerReview.repoId,revision:providerReview.revision,
        filename:file.name,sha256:file.sha256,bytes:file.bytes
      });
      if(reviewed){
        if(reviewed.downloaded!==false||reviewed.state!=='awaiting_p05_transfer'){
          throw Error('Native model review returned an invalid state');
        }
        setProviderTransferIntent(reviewed);
      }
    }catch(error){setProviderFailure(String(error));}
    finally{setProviderBusy(false);}
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
    if (imageViewer?.imageRef===imageRef) setImageViewer(null);
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
  async function inspectSelectedTextFile() {
    if (working || chatBusy) return;
    let targetId = chatId;
    if (!targetId) {
      try {
        const newChat = await invoke('lu_new_chat');
        targetId = newChat.chatId;
        currentChat.current = targetId;
        setChatId(targetId);
        setHistoryWindow({older:null,hasOlder:false,hasNewer:false});
        setHistory([]);
        await refreshChats();
      } catch (error) { setError(String(error)); return; }
    }
    setError('');
    try {
      const result = await invoke('lu_inspect_text_file', {chatId: targetId});
      if (result && currentChat.current === targetId) {
        setHistory(await loadLatestUiHistory(targetId));
        setActivity(result.truncated ? 'File inspected (first 64 KiB)' : 'File inspected');
      }
      if (result) await refreshChats();
    } catch (error) {
      setError(String(error));
      await refreshChats().catch(() => {});
      if (currentChat.current === targetId)
        setHistory(await loadLatestUiHistory(targetId).catch(() => []));
    }
  }

  async function reviewReadProposal(proposal) {
    try {
      const result = await invoke('lu_approve_read_proposal', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      if (result) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await loadLatestUiHistory(proposal.chatId));
        setActivity(result.truncated ? 'Approved native read completed (truncated)' : 'Approved native read completed');
      }
    } catch (error) {
      setError(String(error));
    } finally {
      // The host consumes a proposal on selection, cancellation or rejection.
      setReadProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
    }
  }
  async function rejectReadProposal(proposal) {
    try {
      await invoke('lu_dismiss_read_proposal', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
    } catch (error) { setError(String(error)); }
    setReadProposals(prev => {
      const next = {...prev}; delete next[proposal.proposalId]; return next;
    });
  }

  async function refreshToolActivity(targetChat=chatId){
    if (!targetChat)return;
    setToolHistoryBusy(true);
    try{
      const entries=await invoke('lu_tool_turn_history',{chatId:targetChat});
      setToolHistoryByChat(prev=>({...prev,[targetChat]:entries||[]}));
    }catch(error){
      if (currentChat.current===targetChat)setError('Tool evidence unavailable: '+String(error));
    }finally{setToolHistoryBusy(false)}
  }
  async function refreshWriteRecovery(targetChat = chatId) {
    if (!targetChat) { setWriteRecovery([]); return; }
    const records = await invoke('lu_write_recovery', {chatId: targetChat});
    if (currentChat.current === targetChat) setWriteRecovery(records);
  }
  useEffect(() => {
    if (chatId) {
      refreshWriteRecovery(chatId).catch(() => setWriteRecovery([]));
      refreshEditRecovery(chatId).catch(() => setEditRecovery([]));
      refreshRollbackHistory(chatId).catch(() => setRollbackHistory([]));
      setEditObservations({}); setRollbackReviewed({});
    } else {
      setWriteRecovery([]); setEditRecovery([]);
      setEditObservations({}); setRollbackHistory([]); setRollbackReviewed({});
    }
  }, [chatId]);


  async function refreshEditRecovery(targetChat = chatId) {
    if (!targetChat) { setEditRecovery([]); return; }
    const records = await invoke('lu_edit_recovery',{chatId:targetChat});
    if (currentChat.current === targetChat) setEditRecovery(records);
  }
  async function refreshRollbackHistory(targetChat = chatId) {
    if (!targetChat) { setRollbackHistory([]); return; }
    const records = await invoke('lu_rollback_history',{chatId:targetChat});
    if (currentChat.current === targetChat) setRollbackHistory(records);
  }
  async function inspectEditRecovery(record) {
    try {
      const result = await invoke('lu_inspect_edit_recovery',{
        chatId,proposalId:record.proposalId
      });
      setEditObservations(prev => ({...prev,[record.proposalId]:result}));
      setRollbackReviewed({});
    } catch (error) { setError(String(error)); }
  }
  async function confirmRollback(record) {
    const observation = editObservations[record.proposalId];
    const reviewId = observation?.rollbackReviewId;
    if (!reviewId || !rollbackReviewed[reviewId]) return;
    setError('');
    try {
      const restored = await invoke('lu_confirm_rollback',{chatId,reviewId});
      if (restored) {
        setActivity('Restored previous version: '+restored.fileName);
        setLatestRestored({...restored,chatId});
        await refreshChats();
        if (currentChat.current === chatId)
          setHistory(await loadLatestUiHistory(chatId));
      } else setActivity('Kept current file; restoration denied');
    } catch (error) { setError(String(error)); }
    finally {
      setEditObservations(prev => {const next={...prev};delete next[record.proposalId];return next;});
      setRollbackReviewed(prev => ({...prev,[reviewId]:false}));
      await refreshRollbackHistory(chatId).catch(() => {});
      await refreshEditRecovery(chatId).catch(() => {});
    }
  }
  async function denyRollback(record) {
    const reviewId = editObservations[record.proposalId]?.rollbackReviewId;
    if (!reviewId) return;
    try {
      await invoke('lu_deny_rollback',{chatId,reviewId});
      setActivity('Kept current file; restoration not approved');
      setEditObservations(prev => {const next={...prev};delete next[record.proposalId];return next;});
      setRollbackReviewed(prev => ({...prev,[reviewId]:false}));
      await refreshRollbackHistory(chatId).catch(() => {});
    } catch (error) { setError(String(error)); }
  }
  async function reviewExistingFile(proposal) {
    setError('');
    try {
      const selected = await invoke('lu_review_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      setEditProposals(prev => ({
        ...prev,[proposal.proposalId]:{...prev[proposal.proposalId],selected}
      }));
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
    } catch (error) { setError(String(error)); }
  }
  async function approveExistingEdit(proposal) {
    if (!proposal.selected || !editReviewed[proposal.proposalId]) return;
    setError('');
    try {
      const edited = await invoke('lu_confirm_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      if (edited) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await loadLatestUiHistory(proposal.chatId));
        setActivity('Edited existing file: '+edited.fileName);
        setLatestEdited({...edited,proposalId:proposal.proposalId,chatId:proposal.chatId});
      } else setActivity('Edit denied in native confirmation');
    } catch (error) { setError(String(error)); }
    finally {
      setEditProposals(prev => {const next={...prev};delete next[proposal.proposalId];return next;});
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
      await refreshEditRecovery(proposal.chatId).catch(() => {});
    }
  }
  async function denyExistingEdit(proposal) {
    try {
      await invoke('lu_deny_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      setEditProposals(prev => {const next={...prev};delete next[proposal.proposalId];return next;});
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
      await refreshEditRecovery(proposal.chatId).catch(() => {});
    } catch (error) { setError(String(error)); }
  }
  async function openEditedFolder(proposalId, targetChat = chatId) {
    try {
      await invoke('lu_open_edited_folder', {chatId:targetChat,proposalId});
    } catch (error) { setError(String(error)); }
  }
  async function openCreatedFolder(proposalId, targetChat = chatId) {
    try {
      await invoke('lu_open_created_folder', {chatId: targetChat, proposalId});
    } catch (error) { setError(String(error)); }
  }
  async function chooseDefaultDocumentsFolder() {
    try {
      const result = await invoke('lu_choose_default_document_folder');
      setDocumentSettings(result);
      setActivity('Default document folder updated');
    } catch (error) { setError(String(error)); }
  }
  async function resetDefaultDocumentsFolder() {
    try {
      const result = await invoke('lu_reset_default_document_folder');
      setDocumentSettings(result);
      setActivity('Default restored to system Documents');
    } catch (error) { setError(String(error)); }
  }
  async function rejectNativeActionProposal(proposal){
    try{
      await invoke('lu_deny_native_action_proposal',{
        chatId:proposal.chatId,proposalId:proposal.proposalId,
      });
      setNativeActionProposals(prev=>{
        const next={...prev};delete next[proposal.proposalId];return next;
      });
    }catch(error){setError(String(error));}
  }
  async function reviewNativeActionProposal(proposal){
    if (!chatId||proposal.chatId!==chatId||working||chatBusy)return;
    try{
      const approved=await invoke('lu_accept_native_action_proposal',{
        chatId:proposal.chatId,proposalId:proposal.proposalId,
      });
      setNativeActionProposals(prev=>{
        const next={...prev};delete next[proposal.proposalId];return next;
      });
      // This is NOT effect approval. Every branch opens an existing native
      // picker or a complete draft editor and requires an independent consent.
      switch(approved.action){
        case 'copy_file':await reviewNativeFileTransfer('copy');break;
        case 'move_file':await reviewNativeFileTransfer('move');break;
        case 'trash_file':await reviewNativeTrashFile();break;
        case 'extract_archive':await reviewArchiveTree();break;
        case 'create_document':setDocumentComposerOpen(true);break;
        case 'revise_office':setOfficeRevisionOpen(true);break;
        case 'batch_office':setOfficeBatchOpen(true);break;
        default:throw Error('Unrecognized native operation review');
      }
    }catch(error){setError(String(error));}
  }
  async function runP08Multipart(mode){
    if(!chatId||p08Busy||working||chatBusy)return;
    const owner=chatId;
    const names={
      split:'lu_p08_split_archive',
      resume:'lu_p08_resume_split',
      join:'lu_p08_reassemble_archive',
      review:'lu_p08_review_publication'
    };
    if(!names[mode])return;
    setP08Busy(true);setError('');
    try{
      const receipt=await invoke(names[mode],
        mode==='split'?{chatId:owner,partMib:p08PartMib}:{chatId:owner});
      if(receipt){
        setP08ResultByChat(old=>({...old,[owner]:receipt}));
        setActivity(mode==='review'?'P08 read-only publication status: '+receipt.status:'P08 verified multipart operation published: '+receipt.destination);
      }else{
        setActivity('P08 native archive operation cancelled before publication.');
      }
    }catch(error){
      setError('Multipart archive operation denied or uncertain. Inspect staging; no automatic retry: '+String(error));
    }finally{setP08Busy(false);}
  }
  async function reviewArchiveBatch(){
    if(!chatId||working||chatBusy||archiveBusy||archiveBatchBusy
       ||archiveReview||archiveBatchReview)return;
    setArchiveBatchBusy(true);setArchiveBatchCancelRequested(false);setError('');
    try{
      const review=await invoke('lu_review_archive_batch',{chatId});
      if(review){
        setArchiveBatchReview(review);setArchiveBatchReviewed(false);
        setActivity('Archive batch ready: inspect every member before native approval.');
      }
    }catch(e){setError(String(e));}
    finally{setArchiveBatchBusy(false);}
  }
  async function confirmArchiveBatch(){
    const review=archiveBatchReview;
    if(!review||!archiveBatchReviewed||archiveBatchBusy
       ||review.chatId!==chatId)return;
    setArchiveBatchBusy(true);setError('');setArchiveBatchCancelRequested(false);
    try{
      const result=await invoke('lu_confirm_archive_batch',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if(result){
        setLatestArchiveBatch({...result,chatId:review.chatId});
        setActivity('Archive group created: '+result.groupName);
        await refreshChats();
        if(currentChat.current===review.chatId){
          setHistory(await loadLatestUiHistory(review.chatId));
        }
      }
    }catch(e){
      setError('Archive batch uncertain or refused. Do not automatically retry: '+String(e));
    }finally{
      setArchiveBatchReview(null);setArchiveBatchReviewed(false);
      setArchiveBatchBusy(false);
    }
  }
  async function denyArchiveBatch(){
    const review=archiveBatchReview;
    if(!review||archiveBatchBusy)return;
    try{
      await invoke('lu_deny_archive_batch',{
        chatId:review.chatId,reviewId:review.reviewId
      });
    }catch(e){setError(String(e));}
    finally{setArchiveBatchReview(null);setArchiveBatchReviewed(false);}
  }
  async function cancelArchiveBatch(){
    if(!chatId||!archiveBatchBusy||archiveBatchCancelRequested)return;
    try{
      const stopped=await invoke(
        archiveBatchReview?'lu_cancel_archive_tree':'lu_cancel_archive_preview',
        archiveBatchReview?{chatId,reviewId:archiveBatchReview.reviewId}:{chatId}
      );
      if(stopped){
        setArchiveBatchCancelRequested(true);
        setActivity('Stop requested. An incomplete published group requires manual reconciliation.');
      }
    }catch(e){setError(String(e));}
  }
  async function reviewArchiveTree() {
    if (!chatId || archiveBusy || working || chatBusy) return;
    setArchiveCancelRequested(false);
    setArchiveBusy(true);
    setError('');
    try {
      const review = await invoke('lu_review_archive_tree', {chatId});
      if (review) {
        setArchiveReview(review);
        setArchiveReviewed(false);
        setActivity('Archive review ready — inspect the complete file list before approval');
      }
    } catch (error) { setError(String(error)); }
    finally { setArchiveBusy(false); }
  }
  async function cancelArchivePreview() {
    if (!chatId || !archiveBusy || archiveCancelRequested) return;
    try {
      const requested=await invoke('lu_cancel_archive_preview',{chatId});
      if (requested) {
        setArchiveCancelRequested(true);
        setActivity('Stopping the supervised archive worker. No extraction approval was granted.');
      } else {
        setActivity('Archive picker or preview worker has already finished.');
      }
    } catch (error) {setError(String(error));}
  }
  async function confirmArchiveTree() {
    if (!archiveReview || !archiveReviewed || archiveBusy ||
        archiveReview.chatId !== chatId) return;
    setArchiveBusy(true);
    setArchiveCancelRequested(false);
    setError('');
    const review = archiveReview;
    try {
      const receipt = await invoke('lu_confirm_archive_tree', {
        chatId: review.chatId, reviewId: review.reviewId
      });
      if (receipt) {
        setLatestArchiveExtract({...receipt, chatId: review.chatId});
        setActivity('Archive extracted: ' + receipt.targetFolder);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      } else {
        setActivity('Archive extraction denied in native confirmation');
      }
    } catch (error) {
      setError('Archive extraction was not confirmed. Do not retry an uncertain write: ' + String(error));
    } finally {
      // The host consumes a confirmed review once, even if extraction fails.
      // Never re-submit a possibly partially published archive operation.
      setArchiveReview(null);
      setArchiveReviewed(false);
      setArchiveBusy(false);
    }
  }
  async function requestArchiveCancellation() {
    if (!archiveReview || !archiveBusy || archiveCancelRequested) return;
    try {
      const signalled = await invoke('lu_cancel_archive_tree', {
        chatId: archiveReview.chatId, reviewId: archiveReview.reviewId
      });
      if (signalled) {
        setArchiveCancelRequested(true);
        setActivity('Cancellation requested. The native worker will stop at a safe checkpoint; inspect File operations for recovery.');
      } else {
        setActivity('The extraction worker has not started or has already finished. Use Cancel in the native confirmation dialog if it is open.');
      }
    } catch (error) { setError(String(error)); }
  }
  async function denyArchiveTree() {
    if (!archiveReview || archiveBusy) return;
    setArchiveBusy(true);
    try {
      await invoke('lu_deny_archive_tree', {
        chatId: archiveReview.chatId, reviewId: archiveReview.reviewId
      });
      setArchiveReview(null);
      setArchiveReviewed(false);
      setActivity('Archive extraction denied; no files created');
    } catch (error) { setError(String(error)); }
    finally { setArchiveBusy(false); }
  }

  async function reviewStructuredDocument() {
    if (!chatId || documentCreateBusy || !documentDraft.title.trim() ||
        !documentDraft.content.trim() || documentCreateReview) return;
    setDocumentCreateBusy(true);
    setError('');
    try {
      const view = await invoke('lu_review_document_create', {
        chatId,documentFormat:documentDraft.format,
        title:documentDraft.title,content:documentDraft.content
      });
      if (view) {
        setDocumentCreateReview(view);
        setDocumentCreateReviewed(false);
        setDocumentComposerOpen(false);
        setActivity('Document preview ready — review full content and target');
      }
    } catch (error) { setError(String(error)); }
    finally { setDocumentCreateBusy(false); }
  }
  async function confirmStructuredDocument() {
    if (!documentCreateReview || !documentCreateReviewed ||
        documentCreateBusy || documentCreateReview.chatId !== chatId) return;
    const review = documentCreateReview;
    setDocumentCreateBusy(true);
    setError('');
    try {
      const receipt = await invoke('lu_confirm_document_create', {
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestDocumentCreated({...receipt,chatId:review.chatId});
        setActivity('Created ' + receipt.documentFormat.toUpperCase() + ': ' + receipt.fileName);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      } else {
        setActivity('Document creation denied in native confirmation');
      }
    } catch (error) {
      setError('Document creation not confirmed. Check recovery before retrying: ' + String(error));
    } finally {
      setDocumentCreateReview(null);
      setDocumentCreateReviewed(false);
      setDocumentCreateBusy(false);
    }
  }
  async function denyStructuredDocument() {
    if (!documentCreateReview || documentCreateBusy) return;
    setDocumentCreateBusy(true);
    try {
      await invoke('lu_deny_document_create', {
        chatId:documentCreateReview.chatId,
        reviewId:documentCreateReview.reviewId
      });
      setDocumentCreateReview(null);
      setDocumentCreateReviewed(false);
      setActivity('Document creation denied; no file was written');
    } catch (error) { setError(String(error)); }
    finally { setDocumentCreateBusy(false); }
  }

  async function reviewOfficeRevision() {
    if (!chatId || officeRevisionBusy || officeRevisionReview ||
        !officePatch.expectedOld.trim() || !officePatch.newValue.trim()) return;
    setOfficeRevisionBusy(true);
    setError('');
    try {
      const review=await invoke('lu_review_office_revision', {
        chatId,proposal:officePatch
      });
      if (review) {
        setOfficeRevisionReview(review);
        setOfficeRevisionReviewed(false);
        setOfficeRevisionOpen(false);
        setActivity('Office revision ready — review exact change and preserved package parts');
      }
    } catch (error) { setError(String(error)); }
    finally { setOfficeRevisionBusy(false); }
  }
  async function confirmOfficeRevision() {
    if (!officeRevisionReview || !officeRevisionReviewed ||
        officeRevisionBusy || officeRevisionReview.chatId !== chatId) return;
    const review=officeRevisionReview;
    setOfficeRevisionBusy(true);
    setError('');
    try {
      const receipt=await invoke('lu_confirm_office_revision', {
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestOfficeRevision({...receipt,chatId:review.chatId});
        setActivity('Saved new document revision: ' + receipt.targetNewName);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      } else {
        setActivity('Office revision denied in native confirmation');
      }
    } catch (error) {
      setError('Office revision was not verified. Check recovery before retrying: ' + String(error));
    } finally {
      setOfficeRevisionReview(null);
      setOfficeRevisionReviewed(false);
      setOfficeRevisionBusy(false);
    }
  }
  async function denyOfficeRevision() {
    if (!officeRevisionReview || officeRevisionBusy) return;
    setOfficeRevisionBusy(true);
    try {
      await invoke('lu_deny_office_revision', {
        chatId:officeRevisionReview.chatId,
        reviewId:officeRevisionReview.reviewId
      });
      setOfficeRevisionReview(null);
      setOfficeRevisionReviewed(false);
      setActivity('Office revision denied; original file unchanged');
    } catch (error) { setError(String(error)); }
    finally { setOfficeRevisionBusy(false); }
  }

  async function reviewNativeFileTransfer(operation) {
    if (!chatId || fileTransferBusy || fileTransferReview || working || chatBusy) return;
    setFileTransferBusy(true);
    setError('');
    try {
      const view=await invoke('lu_review_file_transfer', {chatId,operation});
      if (view) {
        setFileTransferReview(view);
        setFileTransferReviewed(false);
        setActivity('Review the exact file content hash and action before approval');
      }
    } catch (error) { setError(String(error)); }
    finally { setFileTransferBusy(false); }
  }
  async function confirmNativeFileTransfer() {
    if (!fileTransferReview || !fileTransferReviewed || fileTransferBusy ||
        fileTransferReview.chatId !== chatId) return;
    const view=fileTransferReview;
    setFileTransferBusy(true);
    setError('');
    try {
      const receipt=await invoke('lu_confirm_file_transfer', {
        chatId:view.chatId,reviewId:view.reviewId
      });
      if (receipt) {
        setLatestFileTransfer({...receipt,chatId:view.chatId});
        setActivity('File ' + receipt.operation + ' completed: ' + receipt.targetName);
        await refreshChats();
        if (currentChat.current === view.chatId)
          setHistory(await loadLatestUiHistory(view.chatId));
      } else {
        setActivity('File operation denied in native confirmation');
      }
    } catch (error) {
      setError('File operation uncertain or rejected. Review before retrying: ' + String(error));
    } finally {
      setFileTransferReview(null);
      setFileTransferReviewed(false);
      setFileTransferBusy(false);
    }
  }
  async function denyNativeFileTransfer() {
    if (!fileTransferReview || fileTransferBusy) return;
    setFileTransferBusy(true);
    try {
      await invoke('lu_deny_file_transfer', {
        chatId:fileTransferReview.chatId,reviewId:fileTransferReview.reviewId
      });
      setFileTransferReview(null);
      setFileTransferReviewed(false);
      setActivity('File operation denied; original file preserved');
    } catch (error) { setError(String(error)); }
    finally { setFileTransferBusy(false); }
  }

  async function refreshTrashHistory(targetChat = chatId) {
    if (!targetChat) return;
    try {
      const records = await invoke('lu_file_trash_history', {chatId:targetChat});
      if (currentChat.current === targetChat) setTrashHistory(records || []);
    } catch (error) { setError('File custody could not be verified: ' + String(error)); }
  }
  async function reviewNativeTrashFile() {
    if (!chatId || trashBusy || trashReview || restoreReview || working || chatBusy) return;
    setTrashBusy(true); setError('');
    try {
      const proposal = await invoke('lu_review_file_trash', {chatId});
      if (proposal) {
        setTrashReview(proposal);
        setTrashReviewed(false);
        setTrashHistoryOpen(true);
      }
    } catch (error) { setError(String(error)); }
    finally { setTrashBusy(false); }
  }
  async function confirmNativeTrash() {
    const review = trashReview;
    if (!review || !trashReviewed || trashBusy || review.chatId!==chatId) return;
    setTrashBusy(true); setError('');
    try {
      const applied=await invoke('lu_confirm_file_trash',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (applied) {
        setLatestCustodyOutcome({...applied,chatId:review.chatId});
        setActivity('Reversible trash recorded: ' + applied.original);
        await refreshTrashHistory(review.chatId);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      } else {
        setActivity('File trash denied in native confirmation');
      }
    } catch(error) {
      setError('Trash was rejected or its effect is uncertain. Inspect custody history; never retry blindly: '+String(error));
      await refreshTrashHistory(review.chatId);
    } finally {
      setTrashReview(null);
      setTrashReviewed(false);
      setTrashBusy(false);
    }
  }
  async function denyNativeTrash() {
    if (!trashReview || trashBusy) return;
    setTrashBusy(true);
    try {
      await invoke('lu_deny_file_trash',{
        chatId:trashReview.chatId,reviewId:trashReview.reviewId
      });
      setTrashReview(null);setTrashReviewed(false);
      setActivity('Reversible file operation denied; no files moved');
    } catch(error){setError(String(error));}
    finally {setTrashBusy(false);}
  }
  async function reviewRecordedRestore(custodyId) {
    if (!chatId || trashBusy || trashReview || restoreReview) return;
    setTrashBusy(true);setError('');
    try {
      const review=await invoke('lu_review_file_restore',{chatId,custodyId});
      setRestoreReview(review);
      setRestoreReviewed(false);
      setTrashHistoryOpen(true);
    } catch(error){setError('Cannot review restore: '+String(error));}
    finally{setTrashBusy(false);}
  }
  async function confirmRecordedRestore() {
    const review=restoreReview;
    if (!review || !restoreReviewed || trashBusy || review.chatId!==chatId) return;
    setTrashBusy(true);setError('');
    try {
      const result=await invoke('lu_confirm_file_restore',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (result) {
        setLatestCustodyOutcome({...result,chatId:review.chatId});
        setActivity('Verified original file restored: '+result.original);
        await refreshTrashHistory(review.chatId);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      } else {
        setActivity('Restore denied in native confirmation');
      }
    } catch(error){
      setError('Restore was rejected or interrupted; inspect custody history before any further write: '+String(error));
      await refreshTrashHistory(review.chatId);
    } finally {
      setRestoreReview(null);
      setRestoreReviewed(false);
      setTrashBusy(false);
    }
  }
  async function denyRecordedRestore() {
    if (!restoreReview || trashBusy) return;
    setTrashBusy(true);
    try {
      await invoke('lu_deny_file_restore',{
        chatId:restoreReview.chatId,reviewId:restoreReview.reviewId
      });
      setRestoreReview(null);setRestoreReviewed(false);
      setActivity('Separate restoration denied');
    } catch(error){setError(String(error));}
    finally {setTrashBusy(false);}
  }

  async function refreshWave2Recovery(targetChat = chatId) {
    if (!targetChat || wave2RecoveryBusy) return;
    setWave2RecoveryBusy(true);
    try {
      const records=await invoke('lu_wave2_recovery',{chatId:targetChat});
      setWave2RecoveryByChat(prev=>({...prev,[targetChat]:records}));
    } catch (error) {
      setError('Cannot inspect Wave 2 operation journal: '+String(error));
    } finally {
      setWave2RecoveryBusy(false);
    }
  }

  function updateOfficeBatchField(index,key,value) {
    setOfficeBatchDraft(previous=>previous.map((patch,i)=>
      i===index ? {...patch,[key]:value} : patch));
  }
  async function reviewOfficeBatch() {
    if (!chatId || officeBatchBusy || officeBatchReview ||
        officeBatchDraft.length < 2 || officeBatchDraft.length > 32 ||
        officeBatchDraft.some(p=>!p.expectedOld.trim() || !p.newValue.trim())) return;
    setOfficeBatchBusy(true);setError('');
    try {
      const review=await invoke('lu_review_office_batch',{
        chatId,proposals:officeBatchDraft
      });
      if (review) {
        setOfficeBatchReview(review);
        setOfficeBatchReviewed(false);
        setOfficeBatchOpen(false);
        setActivity('Multi-edit Office package ready for complete review');
      }
    } catch(error){setError(String(error));}
    finally{setOfficeBatchBusy(false);}
  }
  async function confirmOfficeBatch() {
    const review=officeBatchReview;
    if (!review || !officeBatchReviewed || officeBatchBusy ||
        review.chatId !== chatId) return;
    setOfficeBatchBusy(true);setError('');
    try {
      const receipt=await invoke('lu_confirm_office_batch',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestOfficeBatch({...receipt,chatId:review.chatId});
        setActivity('Created approved multi-edit version: '+receipt.fileName);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await loadLatestUiHistory(review.chatId));
      }else{setActivity('Office batch denied by native confirmation');}
    }catch(error){
      setError('Office batch rejected or uncertain. Inspect recovery before retry: '+String(error));
    }finally{
      setOfficeBatchReview(null);setOfficeBatchReviewed(false);setOfficeBatchBusy(false);
    }
  }
  async function denyOfficeBatch() {
    if (!officeBatchReview || officeBatchBusy) return;
    setOfficeBatchBusy(true);
    try {
      await invoke('lu_deny_office_batch',{
        chatId:officeBatchReview.chatId,reviewId:officeBatchReview.reviewId
      });
      setOfficeBatchReview(null);setOfficeBatchReviewed(false);
      setActivity('Office batch denied; no revision created');
    }catch(error){setError(String(error));}
    finally{setOfficeBatchBusy(false);}
  }

  async function reviewWriteDestination(proposal, destinationMode = 'user_documents') {
    setError('');
    try {
      const destination = await invoke('lu_review_write_destination', {
        chatId: proposal.chatId, proposalId: proposal.proposalId, destinationMode
      });
      setWriteProposals(prev => ({
        ...prev,
        [proposal.proposalId]: {...prev[proposal.proposalId], destination}
      }));
    } catch (error) { setError(String(error)); }
  }
  async function confirmWrite(proposal) {
    setError('');
    try {
      const written = await invoke('lu_confirm_write', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      if (written) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await loadLatestUiHistory(proposal.chatId));
        setActivity('Created new file: ' + written.fileName);
        setLatestCreated({...written, proposalId: proposal.proposalId, chatId: proposal.chatId});
      } else {
        setActivity('File creation denied in native confirmation');
      }
    } catch (error) { setError(String(error)); }
    finally {
      // The trusted host consumes the proposal exactly once, including on
      // failure. Never retry a create automatically.
      setWriteProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
      await refreshWriteRecovery(proposal.chatId).catch(() => {});
    }
  }
  async function denyWrite(proposal) {
    try {
      await invoke('lu_deny_write', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      setWriteProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
    } catch (error) { setError(String(error)); }
  }

  async function askReaderDocument() {
    // This deliberately opts in to a separate read-only structured model
    // tool turn. The native dialog supplies the document, not the WebView.
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    if (images.length || textFiles.length) {
      setError('Ask Reader selects one document with the native picker. Use normal Send for existing attachments.');
      return;
    }
    const input=prompt.trim();
    let activeId=chatId;
    if (!activeId) {
      const created=await invoke('lu_new_chat')
        .catch(error=>{setError(String(error));return null;});
      if (!created) return;
      activeId=created.chatId;
      currentChat.current=activeId;
      setChatId(activeId);
      setHistoryWindow({older:null,hasOlder:false,hasNewer:false});
      setHistory([]);
    }
    setJobs(prev=>({...prev,[activeId]:{
      generationId:null,phase:'selecting-document',position:0,text:''
    }}));
    setError('');
    try {
      const answer=await invoke('lu_reader_ask_document',{
        sessionId:runtime.sessionId,chatId:activeId,prompt:input
      });
      // A canceled OS picker returns null, without making a user turn.
      if (answer!==null && typeof answer!=='string') {
        throw Error('Reader returned an invalid model answer');
      }
      if (answer!==null && currentChat.current===activeId &&
          draftText.current===input) {
        draftText.current='';
        setPrompt('');
        const prior=pendingDrafts.current.get(activeId)||{};
        pendingDrafts.current.set(activeId,{...prior,prompt:''});
      }
    } catch(error) {
      if (currentChat.current===activeId &&
          !/Cancelled|Canceled/i.test(String(error))) {
        setError(String(error));
      }
    } finally {
      await refreshChats().catch(()=>{});
      if (currentChat.current===activeId){
        const entries=await loadLatestUiHistory(activeId).catch(()=>null);
        if (entries&&currentChat.current===activeId)setHistory(entries);
      }
      setJobs(prev=>removeJob(prev,activeId));
    }
  }
  async function askWorkspaceDirectory() {
    // Opt in to one native-selected read-only folder, never a WebView path.
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    if (images.length || textFiles.length) {
      setError('Ask Workspace selects a folder with the native picker. Use normal Send for existing attachments.');
      return;
    }
    const input=prompt.trim();
    let activeId=chatId;
    if (!activeId) {
      const created=await invoke('lu_new_chat')
        .catch(error=>{setError(String(error));return null;});
      if (!created) return;
      activeId=created.chatId;
      currentChat.current=activeId;
      setChatId(activeId);
      setHistoryWindow({older:null,hasOlder:false,hasNewer:false});
      setHistory([]);
    }
    setJobs(prev=>({...prev,[activeId]:{
      generationId:null,phase:'selecting-workspace',position:0,text:''
    }}));
    setError('');
    try {
      const answer=await invoke('lu_workspace_ask_directory',{
        sessionId:runtime.sessionId,chatId:activeId,prompt:input
      });
      if (answer!==null && typeof answer!=='string') {
        throw Error('Workspace returned an invalid model answer');
      }
      if (answer!==null && currentChat.current===activeId &&
          draftText.current===input) {
        draftText.current='';
        setPrompt('');
        const prior=pendingDrafts.current.get(activeId)||{};
        pendingDrafts.current.set(activeId,{...prior,prompt:''});
      }
    } catch(error) {
      if (currentChat.current===activeId &&
          !/Cancelled|Canceled/i.test(String(error))) {
        setError(String(error));
      }
    } finally {
      await refreshChats().catch(()=>{});
      if (currentChat.current===activeId) {
        const entries=await loadLatestUiHistory(activeId).catch(()=>null);
        if(entries&&currentChat.current===activeId)setHistory(entries);
      }
      setJobs(prev=>removeJob(prev,activeId));
    }
  }
  async function send(e) {
    e.preventDefault();
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    const input = prompt.trim();
    const useWorkspaceTools=workspaceToolsNextSend;
    const useBrowserResearch=browserResearchNextSend;
    if(useWorkspaceTools&&useBrowserResearch){
      setError('Choose either workspace tools or Browser research for this turn.');return;
    }
    if(useBrowserResearch&&(images.length||textFiles.length||input.length>12*1024)){
      setError('Browser research requires a text-only question of at most 12 KiB.');return;
    }
    if (useWorkspaceTools && (images.length || textFiles.length)){
      setError('Workspace tools require a text-only question. Remove attachments or turn off workspace tools.');return;
    }
    if (useWorkspaceTools && input.length>8192){
      setError('Workspace tool questions must be at most 8192 characters.');return;
    }
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
      setHistoryWindow({older:null,hasOlder:false,hasNewer:false});
      setHistory([]);
    }
    // A workspace opt-in is consumed on this Send even if the picker is
    // denied/cancelled. No future conversation inherits a filesystem grant.
    if (useWorkspaceTools) setWorkspaceToolsNextSend(false);
    if (useBrowserResearch) setBrowserResearchNextSend(false);
    setImageViewer(null);
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
      const output = useBrowserResearch
        ? await invoke('lu_generate_browser', {
            sessionId:runtime.sessionId,chatId:activeId,
            prompt:input,origins:browserOrigins,maxNewTokens:512
          })
        : await invoke('lu_generate', {
            sessionId: runtime.sessionId, chatId: activeId,
            prompt: submitted, imageRefs: refs, maxNewTokens: 512,
            readOnlyWorkspace: useWorkspaceTools
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
        const entries = await loadLatestUiHistory(activeId)
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
  async function attachReaderDocument(continuePrevious=false) {
    // The local picker is always native and the source is copied into one
    // immutable snapshot; no document path can be passed by the WebView.
    const targetChat=currentChat.current||'new';
    const continuation=continuePrevious?readerContinuation:null;
    if(continuePrevious&&(!continuation||continuation.chatId!==targetChat||
       continuation.selector!==readerSelector||
       continuation.nextOffset===null)){
      setError('No matching Reader source to continue. Select a fresh file.');return;
    }
    try {
      const document=await invoke('lu_reader_pick_document',{
        selector:readerSelector,
        offsetChars:continuation?.nextOffset??0,
        expectedSourceSha256:continuation?.sourceSha256??''
      });
      if (!document) return;
      const prior=pendingDrafts.current.get(targetChat);
      const currentlyVisible=targetChat===(currentChat.current||'new');
      const previous=prior?.files || (currentlyVisible?draftFiles.current:[]);
      if(previous.length>=MAX_TEXT_FILES){
        setError(`You can attach up to ${MAX_TEXT_FILES} documents per message`);
        return;
      }
      const sourceLabel='[Reader extraction: '+document.format+
        (document.selector?'; '+document.selector:'')+
        '; character offset '+document.offsetChars+
        '; source SHA-256 '+document.sourceSha256+']\n';
      const extracted=sourceLabel+document.text+
        (document.truncated?'\n[Reader: more text may remain; continuation is source-pinned]':'');
      const item={
        name:document.name,
        size:document.bytesRead,
        text:extracted,
        format:document.format,
        truncated:document.truncated,
        sourceSha256:document.sourceSha256,
        selector:document.selector,offsetChars:document.offsetChars,
      };
      const next=[...previous,item];
      try{
        buildSubmittedPrompt(prior?.prompt ||
          (currentlyVisible?draftText.current:'Attachment') || 'Attachment',next);
      }catch(error){setError(String(error));return;}
      const old=prior||{
        prompt:currentlyVisible?draftText.current:'',
        images:currentlyVisible?draftImages.current:[],
        files:previous
      };
      pendingDrafts.current.set(targetChat,{...old,files:next});
      if(currentlyVisible){
        draftFiles.current=next;
        setTextFiles(next);
      }
      // If the user changed conversations while a native picker was open,
      // the continuation stays attached to that origin, not to the new chat.
      setReaderContinuation({
        chatId:targetChat,name:document.name,
        selector:document.selector,sourceSha256:document.sourceSha256,
        nextOffset:document.nextOffsetChars,
        sourceTruncated:document.sourceTruncated
      });
      setError('');
    }catch(error){
      setError(String(error));
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
    {historyPaneOpen&&chatId&&<AnsfHistoryPane activeChatId={chatId}
      chats={chats} onClose={()=>setHistoryPaneOpen(false)}/>}
    <main className="workspace">
      {backgroundTerminalNotice && <button type="button"
        className="backgroundGlobalNotice"
        onClick={()=>{setBackgroundPaneOpen(true);setBackgroundFilter('all');}}>
        <span>Background task ended: <strong>{backgroundTerminalNotice.state}</strong></span>
        <small>Open Tasks for details</small>
      </button>}
      <header className="topbar">
        <div className="chatTitle"><h1>{activeChat?.title && activeChat.title !== 'New chat' ? activeChat.title : 'Chat'}</h1>
          <p>{loaded ? 'Ready to chat · local model' : 'Choose a local GGUF to start'}</p></div>
        <div className="headerControls">
          {working && activity.startsWith('Loading') && <span className="busyHint" role="status" aria-live="polite"><span className="spinner" aria-hidden="true"></span> Loading model… {modelLoadSeconds}s elapsed · {lastHostLoadReply && Date.now() - lastHostLoadReply < 5000 ? 'Host responding' : 'Host response pending'}{modelLoadTrace?.loading && modelLoadTrace.events?.length > 0 ? ' · ' + modelLoadTrace.events[modelLoadTrace.events.length - 1].phase.replaceAll('_', ' ') : ''}</span>}
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
          <button type="button" className="subtleButton" title="Open independent ANSF history"
            disabled={!chatId} aria-expanded={historyPaneOpen}
            onClick={()=>setHistoryPaneOpen(v=>!v)}>History library</button>
          <button className="quietSettings" title="Settings" onClick={() => setSettingsOpen(true)}>⚙</button>
        </div>
      </header>
      {chatId&&<section className="m2MemoryPane" aria-label="Conversation goal and clarifications">
        <header className="m2MemoryHeader">
          <strong>Goal memory {m2Context?.goal?'· '+m2Context.goal.slice(0,75):'· not set'}</strong>
          <button type="button" className="subtleButton" aria-expanded={m2PanelOpen}
            onClick={()=>setM2PanelOpen(v=>!v)}>
            {m2PanelOpen?'Hide goal details':'Goal & questions'}
            {m2Context?.questions?.some(q=>q.status==='pending'||q.status==='deferred')
              ?' · needs your input':''}
          </button>
        </header>
        {m2PanelOpen&&<div className="m2MemoryBody">
          <small>This is your chat's persistent task context, not an application permission.
            Only an actual user action can change it. Lu will reuse confirmed answers
            when constructing subsequent local-model context.</small>
          <form onSubmit={m2SetGoal}>
            <label>Current goal
              <textarea rows={2} maxLength={4096} value={m2GoalDraft}
                placeholder="What are we trying to accomplish?"
                onChange={event=>setM2GoalDraft(event.target.value)}/>
            </label>
            <button type="submit" disabled={m2Busy||!m2GoalDraft.trim()||
                m2GoalDraft===m2Context?.goal}>Save goal</button>
          </form>
          {m2Context?.goalId&&<form onSubmit={m2OpenQuestion}>
            <label>Stage a clarification for the user
              <input maxLength={1024} value={m2QuestionDraft}
                placeholder="A yes or no decision to clarify the current goal"
                onChange={event=>setM2QuestionDraft(event.target.value)}/>
            </label>
            <button type="submit" disabled={m2Busy||!m2QuestionDraft.trim()}>
              Add clarification
            </button>
            <small>In M2, questions are user-reviewed. Automatic model-proposed question
              cards are not enabled yet.</small>
          </form>}
          {(m2Context?.questions||[]).slice(-12).map(q=><section key={q.questionId}
            className="m2QuestionCard" aria-label="Goal clarification">
            <strong>{q.question}</strong>
            <small>{q.status==='deferred'?'Not now · you can answer later':
              q.status==='answered'?'Answered: '+q.choice:q.status==='superseded'
              ?'Superseded by a revised goal':'Awaiting an answer'}</small>
            {['pending','deferred'].includes(q.status)&&<>
              <label>Optional comment
                <textarea rows={2} maxLength={1024} value={m2Comments[q.questionId]||''}
                  onChange={e=>setM2Comments(old=>({...old,[q.questionId]:e.target.value}))}
                  placeholder="Add details to your answer"/>
              </label>
              <div className="m2QuestionChoices">
                <button type="button" disabled={m2Busy} onClick={()=>void m2Answer(q.questionId,'yes')}>Yes</button>
                <button type="button" disabled={m2Busy} onClick={()=>void m2Answer(q.questionId,'no')}>No</button>
                <button type="button" className="subtleButton" disabled={m2Busy}
                  onClick={()=>void m2Answer(q.questionId,'not_now')}>Not now</button>
              </div>
            </>}
            {q.status==='answered'&&q.comment&&<small>Your comment: {q.comment}</small>}
          </section>)}
        </div>}
      </section>}
      <div className="transcriptWrap"><div className="messages" ref={transcriptRef} onScroll={onTranscriptScroll} role="log" aria-live="polite" aria-relevant="additions">
        {historyWindow.hasOlder&&<button type="button" className="subtleButton"
          onClick={()=>void loadOlderUiHistory()}>Load older ANSF page</button>}

        {!history.length && <div className="welcome"><div className="welcomeSymbol">L</div>
          <h2>What can I help you build?</h2>
          <p>Local conversations are saved separately. Start a new chat or choose one from your history.</p></div>}
        {displayedHistory.map((entry,i) => <div key={entry.seq??i} data-ansf-seq={entry.seq??undefined} className={'message ' + (entry.role === 'user' ? 'user' : 'assistant')}>
          <span className="messageRole">{entry.role === 'user' ? 'You' : 'Lu'}{entry.cancelled ? ' · stopped' : entry.interrupted ? ' · interrupted' : ''}</span>
          <MessageText text={entry.text} role={entry.role}/></div>)}
        {historyWindow.hasNewer&&<button type="button" className="subtleButton"
          onClick={()=>void loadNewerUiHistory()}>Load newer ANSF page</button>}
        {visibleJob && <div className="message assistant streaming"><span className="messageRole">
          {visibleJob.phase === 'queued' ? `Lu · queued${visibleJob.position ? ' (#' + visibleJob.position + ')' : ''}`
           : visibleJob.phase === 'submitting' ? 'Lu · submitting'
           : visibleJob.cancelling ? 'Lu · stopping'
           : toolProgressByChat[chatId]?.generationId===visibleJob.generationId
             ? toolProgressByChat[chatId]?.phase==='tool_started'
               ? 'Lu · using approved native tool'
               : toolProgressByChat[chatId]?.phase==='tool_returned'
                 ? 'Lu · checking tool result'
                 : 'Lu · thinking with tools'
             : 'Lu · generating'}
          </span>{visibleStream ? <MessageText text={visibleStream}/> : <span className="spinner"/>}
          {turnCapabilitiesByChat[chatId]?.generationId===visibleJob.generationId &&
            <small>Read-only native tools for this turn: {turnCapabilitiesByChat[chatId].tools
              .map(tool=>tool.publicName).join(', ')}</small>}
        </div>}
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
        {chatId && <section className="observePilot" aria-label="Observe and Learn — Lu-only pilot">
          <div className="observePilotTop">
            <strong>Observe & Learn · Lu UI only</strong>
            <span role="status" className={'observeIndicator mode-'+observeView.mode}>
              {observeView.mode==='active'?'● Observing Lu UI events':
               observeView.mode==='paused'?'Paused':'Off'}
            </span>
          </div>
          <small>Experimental · OFF by default · two native confirmations to enable · Linux administrator authentication.
            Observe & Learn data is marked LOCAL ONLY; Lu's network tools must never transmit it.
            You choose what the screen exposes. No remote viewing interface.</small>
          <small>Lu UI event pilot: semantic state changes only · AI prompts issued: 0.
            An additionally approved Windows foreground-window capture rechecks HWND, PID and process creation time;
            the general picker can select a monitor but cannot independently identify the selected OS source. Receipt-only capture fully blanks the frame. Separate user-approved local-GGUF image modes retain one private bounded frame for a deliberate second Attach action.</small>
          <div className="settingsActions">
            {observeView.mode==='off' &&
              <button type="button" onClick={()=>observeAction('start')}>Enable experimental Observe & Learn</button>}
            {observeView.mode==='active' &&
              <button type="button" onClick={()=>observeAction('pause')}>Pause</button>}
            {observeView.mode==='paused' &&
              <button type="button" onClick={()=>observeAction('resume')}>Resume</button>}
            {observeView.mode==='active' && captureWorkerReady &&
              <>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(true)}>
                  {captureStarting?'Capturing…':'Capture selected foreground app (4s)'}
                </button>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(false,true)}>
                  Capture primary display (OS-identified, full screen)
                </button>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(false)}>
                  Select window/display · capture one frame
                </button>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(true,false,true)}>
                  Capture foreground app for local GGUF image
                </button>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(false,true,true)}>
                  Capture primary display for local GGUF image
                </button>
              </>}
            {observeView.mode!=='off' &&
              <button type="button" onClick={()=>observeAction('stop')}>Stop observation</button>}
          </div>
          {observeView.mode==='off' &&
            <button type="button" className="subtleButton"
              onClick={()=>void forgetObserveNotes()}>
              Forget saved observation notes for this chat
            </button>}
          {observeView.mode==='active' && !captureWorkerReady &&
            <small>Windows Graphics Capture is unavailable until the signed-off capture worker is installed with Lu.</small>}
          {captureTaskId && <div aria-label="Native one-shot capture evidence">
            <small role="status">One-shot WGC task: {captureTaskId}.
              Receipts are fully redacted. An explicitly approved local-image
              capture can temporarily hold one bounded frame in native RAM;
              it has no network destination and requires a second action to attach.</small>
            <button type="button" onClick={()=>void reviewCaptureReceipt()}>
              Inspect native frame receipt
            </button>
            {captureLocalTaskId===captureTaskId
              &&captureReceipt?.chatId===chatId
              &&captureReceipt.frameObserved
              &&captureReceipt.sourceIdentityAttested
              &&observeView.mode==='active' &&
              <button type="button" disabled={captureStarting}
                onClick={()=>void attachLocalObservedFrame()}>
                Attach approved frame to this local multimodal chat
              </button>}
            {captureReceipt?.chatId===chatId&&
              <small>Capture state: {captureReceipt.state}.
                {captureReceipt.frameObserved?' WGC frame observed; receipt is fully blanked.':''}
                {captureReceipt.pixelsRetained
                  ?' A private local-only frame is temporarily held in native RAM pending Attach or cancellation.'
                  :''}
                {captureReceipt.frameSha256
                  ?' Fully redacted frame receipt SHA-256: '+captureReceipt.frameSha256
                  :' No completed frame digest.'}
                {captureReceipt.sourceIdentityAttested
                  ?' Native foreground HWND or primary display identity checked.'
                  :' Selected source identity is not attested.'}
                {captureReceipt.fullyRedactedReceipt
                  ?' Entire captured frame was blanked before receipt output.'
                  :' No verified redacted frame receipt.'}
                This is not an image preview, GUI test verdict or model visual understanding.</small>}
          </div>}
          {observeView.mode!=='off'&&
            <small>Observed state-change hints: {observeView.nativeEventCount||0}.
              Durable awareness notes: {observeView.notesLogged||0}.
              No automatically saved learning, GPU inference or GUI test verdict.</small>}
          {observeView.mode!=='off'&&
            <div className="observeNotations" aria-label="Local awareness notations">
              <strong>Awareness notes · native-only</strong>
              {observeNotes.length===0
                ? <small>No issues noted in this observation session.</small>
                : <ul>{observeNotes.slice(0,8).map(n=>
                  <li key={n.sequence}>
                    {n.kind==='error_indicator'?'Error indicator observed (not verified)':
                      n.kind==='verified_test_failure'?'Independently verified test failure':
                      n.kind} · {n.source==='renderer_hint'?'UI hint':n.source}
                    · {n.priority}
                  </li>
                )}</ul>}
              <button type="button" disabled={observeNotes.length===0}
                onClick={reviewObserveHandoff}>Prepare large-model review handoff</button>
              <button type="button" className="subtleButton"
                onClick={()=>void forgetObserveNotes()}>
                Forget observation notes for this chat
              </button>
              <small>Diagnostic codes expire after seven days. Forgetting clears Lu's
                stored notes and current review summary, not existing chat messages,
                exported text, or durable task-operation receipts. No observation data
                is automatically promoted to memory or model training.</small>
              {observeHandoff?.sessionId===observeView.sessionId&&
                <div role="status" className="observeHandoff">
                  <strong>Observed evidence prepared for human review</strong>
                  <small>{observeHandoff.sourceNoteCount} bounded native records.
                    Independently verified test failures: {observeHandoff.verifiedFailures};
                    unverified indicators: {observeHandoff.unverifiedIndicators}.</small>
                  <small>Review SHA-256: {observeHandoff.reviewSha256}</small>
                  <textarea readOnly rows={8} aria-label="Read-only native observation handoff"
                    value={observeHandoff.reviewDraft}/>
                  <div className="settingsActions">
                    <button type="button" disabled={!chatId||working||chatBusy}
                      onClick={()=>{
                        const draft=observeHandoff.reviewDraft;
                        if(!draft||observeHandoff.sessionId!==observeView.sessionId)return;
                        setPrompt(prev=>prev?(prev+'\n\n'+draft):draft);
                        setObserveHandoff(null);
                        setActivity('Observation briefing added to chat draft. Review and press Send when ready.');
                      }}>
                      Add observed evidence to chat draft
                    </button>
                    <button type="button" onClick={()=>setObserveHandoff(null)}>
                      Dismiss briefing
                    </button>
                  </div>
                  <small>Nothing is sent automatically. These records never establish
                    verified GUI correctness or grant source editing, OS access,
                    or further tools; ordinary per-turn approval still applies.</small>
                </div>}
            </div>}
        </section>}
        {wave2FileBusy && <section className="readProposalCard" role="status"
          aria-live="polite" aria-label="Native file operation in progress">
          <strong>Native file operation in progress</strong>
          <small>Lu is handling a native picker, confirmation, bounded document/archiving work,
            or receipt verification. Byte-level progress is not available in this workflow.
            No automatic retry occurs if a write is interrupted; inspect File operations or
            File recovery before taking another action.</small>
        </section>}
        {error && <div className="error" role="alert"><span>{error}</span><button onClick={() => setError('')}>Dismiss</button></div>}
        {Object.values(readProposals).filter(proposal => proposal.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard" role="group"
            aria-label="Model proposed file read">
            <strong>Lu requests a file read</strong>
            <p>{proposal.purpose}</p>
            <small>Requested filename: {proposal.fileName} · Up to {proposal.maxBytes} bytes.
              This request has no authorization until you select the matching file.</small>
            <div className="settingsActions">
              <button type="button" className="primaryButton" onClick={() => reviewReadProposal(proposal)}>
                Choose and approve file
              </button>
              <button type="button" className="subtleButton" onClick={() => rejectReadProposal(proposal)}>
                Deny
              </button>
            </div>
          </section>
        )}
        {Object.values(nativeActionProposals)
          .filter(proposal=>proposal.chatId===chatId).map(proposal=>
            <section key={proposal.proposalId} className="readProposalCard"
              role="group" aria-label="Lu suggested native action review">
              <strong>Review suggested action: {proposal.action.replaceAll('_',' ')}</strong>
              <p>{proposal.purpose}</p>
              <small>This model suggestion cannot change files. The native
                picker and a separate approval are required before execution.</small>
              <div className="settingsActions">
                <button type="button" className="primaryButton"
                  disabled={working||chatBusy}
                  onClick={()=>void reviewNativeActionProposal(proposal)}>
                  Review with native controls
                </button>
                <button type="button" className="subtleButton"
                  onClick={()=>void rejectNativeActionProposal(proposal)}>Deny</button>
              </div>
            </section>)}
        {toolActivityOpen && <section className="readProposalCard"
          role="region" aria-label="Read-only native tool activity">
          <strong>Native tool activity</strong>
          <small>Host-recorded Reader and Workspace tool-turn evidence. No
            permission is restored by inspecting this history; write effects
            remain in the separate File operations recovery panel.</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={toolHistoryBusy}
              onClick={()=>void refreshToolActivity(chatId)}>
              {toolHistoryBusy?'Refreshing…':'Refresh tool activity'}
            </button>
            <button type="button" className="subtleButton"
              onClick={()=>setToolActivityOpen(false)}>Close</button>
          </div>
          {(toolHistoryByChat[chatId]||[]).length===0&&
            <small>No persisted native model/tool events in this conversation.</small>}
          {(toolHistoryByChat[chatId]||[]).map(item=>
            <div key={item.source+':'+item.taskId} className="readProposalCard"
              role="group" aria-label={'Tool audit '+item.taskId}>
              <strong>{item.source} — {item.state.replaceAll('_',' ')}</strong>
              <small>{item.recordedAt} · Task {item.taskId}</small>
              <small>{item.modelResponses} model responses ·
                {item.toolFeedback} tool results ·
                {item.rejectedEvents} rejections ·
                {item.repairedResponses} structured repairs</small>
            </div>)}
        </section>}
        {(wave2RecoveryByChat[chatId] || []).length > 0 && <section
          className="readProposalCard" role="region" aria-label="Durable Wave 2 operation recovery">
          <strong>File and document operation history</strong>
          <small>Read-only records reloaded from Lu's durable host intent and effect journals.
            A matched receipt confirms a past journaled effect, not that no external process
            has subsequently changed the files.</small>
          {(wave2RecoveryByChat[chatId] || []).map(operation=>
            <div key={operation.effectCallId} className="readProposalCard">
              <strong>{operation.summary} — {operation.state.replaceAll('_',' ')}</strong>
              <code className="writeTarget">{operation.exactTarget}</code>
              <small>Tool: {operation.toolId}</small>
              <small>Approved SHA-256: {operation.approvedSha256}</small>
              <small>Effect ID: {operation.effectCallId}</small>
              <small>{operation.assessment}</small>
              {operation.targetIntegrity && operation.targetIntegrity !== 'NOT_RECHECKED' &&
                <small>Current output integrity: {operation.targetIntegrity}</small>}
              {!operation.receiptMatched &&
                <small>Do not automatically retry. Reconcile the filesystem and journal first.</small>}
            </div>
          )}
        </section>}
        {trashReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group" aria-label="Review reversible file trash">
          <strong>Move the EXACT selected file into Lu's reversible custody</strong>
          <small>Native-selected original file:</small>
          <code className="writeTarget">{trashReview.source}</code>
          <small>Lu-generated custody filename, in the same directory:</small>
          <code className="writeTarget">{trashReview.storedName}</code>
          <small>{trashReview.bytes} bytes · SHA-256 {trashReview.sha256}</small>
          <small>The original filename disappears on success. The same bytes are retained
            under a new Lu custody name in the same folder, NOT the system Recycle Bin.
            Restoration requires a separate human approval.</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={trashReviewed}
              onChange={e=>setTrashReviewed(e.target.checked)}/>
            I reviewed the exact file, checksum, custody filename, and reversible rename.
          </label>
          <small>An independent native confirmation still controls the write. Interrupted
            actions are never automatically retried.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!trashReviewed || trashBusy}
              onClick={()=>void confirmNativeTrash()}>Approve trash in native dialog</button>
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void denyNativeTrash()}>Deny trash</button>
          </div>
        </section>}
        {restoreReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group" aria-label="Review recorded file restoration">
          <strong>Restore a journal-verified file to its ORIGINAL filename</strong>
          <small>Custody ID: {restoreReview.custodyId}</small>
          <small>Exact restoration destination:</small>
          <code className="writeTarget">{restoreReview.original}</code>
          <small>Existing custody name: {restoreReview.trashEntry}</small>
          <small>{restoreReview.bytes} bytes · SHA-256 {restoreReview.sha256}</small>
          <small>Lu will never overwrite an existing file. Only a previously applied native
            custody receipt is eligible for restore. This requires a new one-use authorization.</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={restoreReviewed}
              onChange={e=>setRestoreReviewed(e.target.checked)}/>
            I reviewed the exact restoration destination, verified custody hash, and no-overwrite rule.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!restoreReviewed || trashBusy}
              onClick={()=>void confirmRecordedRestore()}>Approve restore in native dialog</button>
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void denyRecordedRestore()}>Deny restore</button>
          </div>
        </section>}
        {latestCustodyOutcome?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Reversible file operation: {latestCustodyOutcome.state}</strong>
          <small>Original filename: {latestCustodyOutcome.original}</small>
          <small>{latestCustodyOutcome.bytes} bytes · SHA-256 {latestCustodyOutcome.sha256}</small>
          <small>Custody ID: {latestCustodyOutcome.custodyId}</small>
          <small>Native effect receipt: {latestCustodyOutcome.effectCallId}</small>
        </section>}
        {trashHistoryOpen && chatId && <section className="readProposalCard"
          role="region" aria-label="Durable file custody and recovery">
          <strong>File custody and recovery</strong>
          <small>Read-only inspection of your journaled file trash and restore operations.
            Status is rechecked against stored receipts and the current filesystem.
            Uncertain operations require manual reconciliation; no automatic retry.</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void refreshTrashHistory()}>Refresh custody status</button>
            <button type="button" className="subtleButton"
              onClick={()=>setTrashHistoryOpen(false)}>Close history</button>
          </div>
          {!trashHistory.length && <small>No durable file custody records for this chat.</small>}
          {trashHistory.map(item=><div key={item.custodyId} className="readProposalCard"
            role="group" aria-label={'Custody for '+item.original}>
            <strong>{item.original} — {item.state.replaceAll('_',' ')}</strong>
            <small>{item.bytes} bytes · SHA-256 {item.sha256}</small>
            <small>Observation: {item.observation.replaceAll('_',' ')}</small>
            <small>Trash receipt: {item.trashEffectId}</small>
            {item.restoreEffectId && <small>Restore receipt: {item.restoreEffectId}</small>}
            {item.restorable ?
              <button type="button" className="primaryButton"
                disabled={trashBusy || Boolean(restoreReview) || Boolean(trashReview)}
                onClick={()=>void reviewRecordedRestore(item.custodyId)}>
                Review restoration
              </button>
              : <small>{item.state==='UNCERTAIN' || item.observation.includes('NOT_')
                  ? 'No repeat or automatic recovery is authorized for this record.'
                  : 'Restore is not currently available for this record.'}</small>}
          </div>)}
        </section>}
        {fileTransferReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Review exact file copy or move">
          <strong>{fileTransferReview.operation==='move' ? 'MOVE existing file' : 'COPY existing file'}
            into a NEW filename</strong>
          <small>Native-picked source:</small>
          <code className="writeTarget">{fileTransferReview.sourceDisplay}</code>
          <strong>Host-generated new destination (never overwrites)</strong>
          <code className="writeTarget">{fileTransferReview.destinationDisplay}</code>
          <small>{fileTransferReview.bytes} bytes · source SHA-256: {fileTransferReview.sourceSha256}</small>
          <small>{fileTransferReview.removesOriginal
            ? 'MOVE removes the original name after linking the new one. A failed unlink requires manual reconciliation.'
            : 'COPY keeps the original file unchanged.'}</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={fileTransferReviewed}
              onChange={e=>setFileTransferReviewed(e.target.checked)}/>
            I reviewed the source, destination, SHA-256 and {fileTransferReview.operation} consequences.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!fileTransferReviewed || fileTransferBusy}
              onClick={()=>void confirmNativeFileTransfer()}>Approve in native confirmation</button>
            <button type="button" className="subtleButton" disabled={fileTransferBusy}
              onClick={()=>void denyNativeFileTransfer()}>Deny operation</button>
          </div>
        </section>}
        {latestFileTransfer?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>File {latestFileTransfer.operation} complete: {latestFileTransfer.targetName}</strong>
          <code className="writeTarget">{latestFileTransfer.exactDestination}</code>
          <small>{latestFileTransfer.bytes} bytes · SHA-256: {latestFileTransfer.sha256}</small>
          <small>{latestFileTransfer.sourceRemoved
            ? 'Original filename removed.'
            : 'Original file retained.'} Effect receipt: {latestFileTransfer.effectCallId}</small>
        </section>}
        {officeBatchOpen && !officeBatchReview && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Prepare a multi-edit Office revision">
          <strong>Revise multiple Word, PowerPoint or Excel fields in one new version</strong>
          <small>Select the original document and a NEW output folder through native pickers.
            Every existing value must match. No modifications are made until final native approval.</small>
          <small role="note">P09 Office fidelity boundary: only 2–32 exact text runs or simple typed cells.
            Lu preserves unchanged OOXML part content, not ZIP container bytes or guaranteed
            rendering/layout fidelity. Formulas are not recalculated; charts, embedded objects,
            macros, signatures and external links are not editable here.</small>
          {officeBatchDraft.map((patch,i)=><fieldset key={i} className="readProposalCard">
            <legend>Change {i+1} of {officeBatchDraft.length}</legend>
            <label className="editReviewAck">Kind
              <select value={patch.kind} onChange={e=>updateOfficeBatchField(i,'kind',e.target.value)}>
                <option value="word_text">Word text run</option>
                <option value="slide_text">PowerPoint slide text</option>
                <option value="cell_number">Excel numeric cell</option>
                <option value="cell_boolean">Excel boolean cell</option>
                <option value="cell_text">Excel inline text cell</option>
              </select>
            </label>
            {(patch.kind==='word_text'||patch.kind==='slide_text') &&
              <label className="editReviewAck">Text run index (zero based)
                <input type="number" min="0" max="9999" value={patch.textIndex}
                  onChange={e=>updateOfficeBatchField(i,'textIndex',
                    Math.max(0,Number(e.target.value)||0))}/>
              </label>}
            {patch.kind==='slide_text' && <label className="editReviewAck">Slide number
              <input type="number" min="1" max="256" value={patch.slideNumber}
                onChange={e=>updateOfficeBatchField(i,'slideNumber',
                  Math.max(1,Number(e.target.value)||1))}/>
            </label>}
            {patch.kind.startsWith('cell_') && <>
              <label className="editReviewAck">Worksheet number
                <input type="number" min="1" max="256" value={patch.sheetNumber}
                  onChange={e=>updateOfficeBatchField(i,'sheetNumber',
                    Math.max(1,Number(e.target.value)||1))}/>
              </label>
              <label className="editReviewAck">Cell address
                <input maxLength={8} value={patch.address}
                  onChange={e=>updateOfficeBatchField(i,'address',e.target.value.toUpperCase())}/>
              </label>
            </>}
            <label className="editReviewAck">Expected old value
              <textarea className="editFullText" rows={2} maxLength={32768}
                value={patch.expectedOld}
                onChange={e=>updateOfficeBatchField(i,'expectedOld',e.target.value)}/>
            </label>
            <label className="editReviewAck">New value
              <textarea className="editFullText" rows={2} maxLength={32768}
                value={patch.newValue}
                onChange={e=>updateOfficeBatchField(i,'newValue',e.target.value)}/>
            </label>
            <button type="button" className="subtleButton"
              disabled={officeBatchDraft.length<=2}
              onClick={()=>setOfficeBatchDraft(prev=>prev.filter((_,j)=>j!==i))}>
              Remove change
            </button>
          </fieldset>)}
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={officeBatchDraft.length>=32}
              onClick={()=>setOfficeBatchDraft(prev=>[...prev,{
                kind:prev[0]?.kind||'word_text',textIndex:prev.length,
                slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''
              }])}>Add change</button>
            <button type="button" className="primaryButton"
              disabled={officeBatchBusy || officeBatchDraft.some(p=>!p.expectedOld.trim()||!p.newValue.trim())}
              onClick={()=>void reviewOfficeBatch()}>Native-pick source and review full batch</button>
            <button type="button" className="subtleButton"
              onClick={()=>setOfficeBatchOpen(false)}>Close</button>
          </div>
        </section>}
        {officeBatchReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Review all Office revision changes and unchanged package parts">
          <strong>Approve an exact {officeBatchReview.changes.length}-change Office version</strong>
          <small>Original document:</small>
          <code className="writeTarget">{officeBatchReview.sourceDisplay}</code>
          <small>New version destination (never overwritten):</small>
          <code className="writeTarget">{officeBatchReview.destinationDisplay}</code>
          <small>Original SHA-256: {officeBatchReview.originalSha256}</small>
          <small>Approved new package SHA-256: {officeBatchReview.revisedSha256}</small>
          <small>{officeBatchReview.changedParts} · {officeBatchReview.revisedBytes} bytes</small>
          <strong>All ordered exact changes</strong>
          {officeBatchReview.changes.map((change,i)=><div key={i} className="readProposalCard">
            <strong>{i+1}. {change.kind}: {change.target}</strong>
            <small>Before</small>
            <pre className="editFullText">{change.expectedOld}</pre>
            <small>After</small>
            <pre className="editFullText">{change.newValue}</pre>
          </div>)}
          <details open>
            <summary>Every unchanged package part ({officeBatchReview.unaffectedParts.length})</summary>
            <pre className="editFullText" aria-label="All unaffected Office part hashes">
              {officeBatchReview.unaffectedParts.map(p=>p[0]+' · '+p[1]).join('\n')}
            </pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={officeBatchReviewed}
              onChange={e=>setOfficeBatchReviewed(e.target.checked)}/>
            I reviewed every before/after change, exact file paths, and whole-package hashes.
          </label>
          <small>This batch is all-or-nothing before publication. The original remains unchanged.
            A separate native confirmation is required for this single-use write.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!officeBatchReviewed || officeBatchBusy}
              onClick={()=>void confirmOfficeBatch()}>Approve full batch in native dialog</button>
            <button type="button" className="subtleButton" disabled={officeBatchBusy}
              onClick={()=>void denyOfficeBatch()}>Deny all edits</button>
          </div>
        </section>}
        {latestOfficeBatch?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Office multi-edit version created: {latestOfficeBatch.fileName}</strong>
          <code className="writeTarget">{latestOfficeBatch.exactDestination}</code>
          <small>{latestOfficeBatch.bytesWritten} bytes · SHA-256: {latestOfficeBatch.sha256}</small>
          <small>Durable journal receipt: {latestOfficeBatch.effectCallId}</small>
        </section>}
        {officeRevisionOpen && !officeRevisionReview && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Prepare an existing Office document revision">
          <strong>Revise existing Word, PowerPoint, or Excel document</strong>
          <small>The original file is selected with a native picker and never overwritten.
            A second native picker chooses the folder for a NEW versioned revision.</small>
          <label className="editReviewAck">Change type
            <select value={officePatch.kind}
              onChange={e=>setOfficePatch(prev=>({...prev,kind:e.target.value}))}>
              <option value="word_text">Word: text run</option>
              <option value="slide_text">PowerPoint: slide text run</option>
              <option value="cell_number">Excel: numeric cell</option>
              <option value="cell_boolean">Excel: boolean cell</option>
              <option value="cell_text">Excel: inline text cell</option>
            </select>
          </label>
          {(officePatch.kind==='word_text' || officePatch.kind==='slide_text') && <label className="editReviewAck">
            Text run index (0-based)
            <input type="number" min="0" max="9999" value={officePatch.textIndex}
              onChange={e=>setOfficePatch(prev=>({...prev,textIndex:Math.max(0,Number(e.target.value)||0)}))}/>
          </label>}
          {officePatch.kind==='slide_text' && <label className="editReviewAck">
            Slide number (1-based)
            <input type="number" min="1" max="256" value={officePatch.slideNumber}
              onChange={e=>setOfficePatch(prev=>({...prev,slideNumber:Math.max(1,Number(e.target.value)||1)}))}/>
          </label>}
          {officePatch.kind.startsWith('cell_') && <>
            <label className="editReviewAck">Worksheet number (1-based)
              <input type="number" min="1" max="256" value={officePatch.sheetNumber}
                onChange={e=>setOfficePatch(prev=>({...prev,sheetNumber:Math.max(1,Number(e.target.value)||1)}))}/>
            </label>
            <label className="editReviewAck">Cell address
              <input value={officePatch.address} maxLength={8}
                onChange={e=>setOfficePatch(prev=>({...prev,address:e.target.value.toUpperCase()}))}
                placeholder="B2"/>
            </label>
          </>}
          <label className="editReviewAck">Expected existing value (must match exactly)
            <textarea rows={2} className="editFullText" maxLength={32768}
              value={officePatch.expectedOld} onChange={e=>
                setOfficePatch(prev=>({...prev,expectedOld:e.target.value}))}/>
          </label>
          <label className="editReviewAck">New value
            <textarea rows={2} className="editFullText" maxLength={32768}
              value={officePatch.newValue} onChange={e=>
                setOfficePatch(prev=>({...prev,newValue:e.target.value}))}/>
          </label>
          <small>Unsupported formulas, signatures, macros, rich objects or unsafe structures are refused.
            Office revisions are new files; they never overwrite the selected original.</small>
          <small role="note">P09 fidelity: this changes one exact text run or simple typed cell,
            not entire paragraphs, tables or rich text. Unchanged package PART CONTENT is
            retained; ZIP metadata, visual layout and Office application rendering are not
            certified. Check the new version in a dedicated Office application.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!chatId || officeRevisionBusy || !officePatch.expectedOld.trim() ||
                !officePatch.newValue.trim()}
              onClick={()=>void reviewOfficeRevision()}>Choose source & output folder</button>
            <button type="button" className="subtleButton"
              onClick={()=>setOfficeRevisionOpen(false)}>Close</button>
          </div>
        </section>}
        {officeRevisionReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard"
          role="group" aria-label="Review exact Office document revision">
          <strong>New Office revision; original stays unchanged</strong>
          <small>Original source (SHA-256 {officeRevisionReview.originalSha256}):</small>
          <code className="writeTarget">{officeRevisionReview.sourceDisplay}</code>
          <small>New output (SHA-256 {officeRevisionReview.revisedSha256}):</small>
          <code className="writeTarget">{officeRevisionReview.destinationDisplay}</code>
          <small>Changed package part: {officeRevisionReview.targetPart}.
            Total revised document size: {officeRevisionReview.revisedBytes} bytes.</small>
          <strong>Exact proposed change</strong>
          <small>Before</small>
          <pre className="editFullText">{officeRevisionReview.originalValue}</pre>
          <small>After</small>
          <pre className="editFullText">{officeRevisionReview.newValue}</pre>
          <details open>
            <summary>All unchanged package parts ({officeRevisionReview.unaffectedParts.length})</summary>
            <pre className="editFullText" aria-label="Preserved Office package part checksums">
              {officeRevisionReview.unaffectedParts.map(x => x.partName + ' · ' + x.sha256).join('\n')}
            </pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={officeRevisionReviewed}
              onChange={e=>setOfficeRevisionReviewed(e.target.checked)}/>
            I reviewed the exact change, original and new paths, and all preserved package parts.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!officeRevisionReviewed || officeRevisionBusy}
              onClick={()=>void confirmOfficeRevision()}>Approve new revision in native dialog</button>
            <button type="button" className="subtleButton" disabled={officeRevisionBusy}
              onClick={()=>void denyOfficeRevision()}>Deny revision</button>
          </div>
        </section>}
        {latestOfficeRevision?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Office document revision created: {latestOfficeRevision.targetNewName}</strong>
          <code className="writeTarget">{latestOfficeRevision.exactDestination}</code>
          <small>{latestOfficeRevision.bytesWritten} bytes · SHA-256 {latestOfficeRevision.sha256}</small>
          <small>Effect receipt: {latestOfficeRevision.effectCallId}. Original file preserved.</small>
        </section>}
        {documentComposerOpen && !documentCreateReview && <section
          className="readProposalCard editProposalCard" aria-label="Create structured Office document">
          <strong>Create a new structured Word or PowerPoint document</strong>
          <small>Lu will generate the file from these reviewed words, but only your native folder picker
            and independent native confirmation may authorize writing it.</small>
          <label className="editReviewAck">Document format
            <select value={documentDraft.format} onChange={e =>
              setDocumentDraft(prev=>({...prev,format:e.target.value}))}>
              <option value="docx">Word DOCX</option>
              <option value="pptx">PowerPoint PPTX</option>
            </select>
          </label>
          <label className="editReviewAck">Document title
            <input value={documentDraft.title} maxLength={256}
              onChange={e=>setDocumentDraft(prev=>({...prev,title:e.target.value}))}
              placeholder="Document or presentation title"/>
          </label>
          <label className="editReviewAck">Complete text
            <textarea className="editFullText" rows={8} maxLength={16384}
              value={documentDraft.content}
              onChange={e=>setDocumentDraft(prev=>({...prev,content:e.target.value}))}
              placeholder={documentDraft.format==='pptx'
                ? 'Bullet lines on slide one\n\nSlide two title\nBullet lines on slide two'
                : 'First paragraph\nSecond paragraph'}/>
          </label>
          <small>{documentDraft.format==='pptx'
            ? 'For presentations, blank lines divide slides. A single block uses the document title for slide one.'
            : 'For Word, each nonempty line becomes a paragraph after the title.'}</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton" disabled={documentCreateBusy ||
              !chatId || !documentDraft.title.trim() || !documentDraft.content.trim()}
              onClick={() => void reviewStructuredDocument()}>Choose output folder and review</button>
            <button type="button" className="subtleButton"
              onClick={() => setDocumentComposerOpen(false)}>Close</button>
          </div>
        </section>}
        {documentCreateReview?.chatId === chatId && <section className="readProposalCard editProposalCard"
          role="group" aria-label="Review complete structured document creation">
          <strong>New {documentCreateReview.documentFormat.toUpperCase()} document (no overwrite)</strong>
          <code className="writeTarget">{documentCreateReview.exactDestination}</code>
          <small>{documentCreateReview.bytes} bytes · {documentCreateReview.contentUnits} paragraphs/slides</small>
          <small>Exact package SHA-256: {documentCreateReview.sha256}</small>
          <strong>Title: {documentCreateReview.title}</strong>
          <small>Review ALL proposed content before confirming:</small>
          <pre className="editFullText" aria-label="All proposed document text">{documentCreateReview.content}</pre>
          <label className="editReviewAck">
            <input type="checkbox" checked={documentCreateReviewed}
              onChange={e => setDocumentCreateReviewed(e.target.checked)}/>
            I reviewed the entire new document content, exact file destination, and package hash.
          </label>
          <small>A separate native confirmation still grants the single-use write. Existing files
            are never replaced. Interrupted writes require recovery review.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!documentCreateReviewed || documentCreateBusy}
              onClick={() => void confirmStructuredDocument()}>Confirm in native dialog</button>
            <button type="button" className="subtleButton" disabled={documentCreateBusy}
              onClick={() => void denyStructuredDocument()}>Deny document creation</button>
          </div>
        </section>}
        {latestDocumentCreated?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Created {latestDocumentCreated.documentFormat.toUpperCase()}:
            {latestDocumentCreated.fileName}</strong>
          <code className="writeTarget">{latestDocumentCreated.exactDestination}</code>
          <small>{latestDocumentCreated.bytesWritten} bytes · SHA-256: {latestDocumentCreated.sha256}</small>
          <small>Durable effect receipt: {latestDocumentCreated.effectCallId}</small>
        </section>}
        {archiveBatchReview?.chatId===chatId&&
          <section className="readProposalCard editProposalCard" role="group"
            aria-label="Review multiple archive contents">
            <strong>Multi-archive batch — NEW group only</strong>
            <small>Native pickers selected {archiveBatchReview.archiveCount} files.
              Nothing has been extracted. Review every member before confirming.</small>
            <code className="writeTarget">{archiveBatchReview.groupDisplay}</code>
            <small>{archiveBatchReview.totalFiles} files ·
              {' '}{archiveBatchReview.totalBytes} expanded bytes</small>
            <small>Entire approved batch SHA-256: {archiveBatchReview.manifestSha256}</small>
            <details open>
              <summary>All archive members and source digests</summary>
              {archiveBatchReview.items.map((item,i)=><div key={i}>
                <strong>{item.source} → {item.outputFolder}</strong>
                <small>{item.files} files · {item.expandedBytes} bytes ·
                  {' '}{item.manifestSha256}</small>
                <pre className="editFullText">{item.entries.map((entry,j)=>
                  (j+1)+'. '+(entry.is_directory?'directory':'file')+' · '+
                  entry.path+(entry.is_directory?'':' · '+
                    entry.uncompressed_bytes+' bytes · '+entry.sha256)
                ).join('\n')}</pre>
              </div>)}
            </details>
            <label className="editReviewAck">
              <input type="checkbox" checked={archiveBatchReviewed}
                onChange={e=>setArchiveBatchReviewed(e.target.checked)}/>
              I reviewed every archive entry, all destination folders, and
              the manual recovery rule for any partial write.
            </label>
            <div className="settingsActions">
              <button type="button" className="primaryButton"
                disabled={!archiveBatchReviewed||archiveBatchBusy}
                onClick={()=>void confirmArchiveBatch()}>
                Confirm entire batch using native approval
              </button>
              <button type="button" disabled={archiveBatchBusy}
                onClick={()=>void denyArchiveBatch()}>Deny batch</button>
              {archiveBatchBusy&&<button type="button"
                disabled={archiveBatchCancelRequested}
                onClick={()=>void cancelArchiveBatch()}>
                {archiveBatchCancelRequested?'Stopping…':'Cancel batch'}
              </button>}
            </div>
          </section>}
        {latestArchiveBatch?.chatId===chatId&&
          <section className="readProposalCard" role="status">
            <strong>Archive batch extracted into {latestArchiveBatch.groupName}</strong>
            <small>{latestArchiveBatch.archiveCount} sources ·
              {' '}{latestArchiveBatch.totalFiles} files ·
              {' '}{latestArchiveBatch.totalBytes} bytes</small>
            <small>Verified batch SHA-256: {latestArchiveBatch.manifestSha256}</small>
            <small>Durable effect: {latestArchiveBatch.effectCallId}.
              Original archives preserved.</small>
          </section>}
        {archiveReview?.chatId === chatId && <section className="readProposalCard editProposalCard"
          role="group" aria-label="Review complete archive extraction">
          <strong>Extract archive into a NEW folder</strong>
          <small>Trusted OS file picker selected this archive; Lu has not extracted anything.</small>
          <code className="writeTarget">{archiveReview.sourceDisplay}</code>
          <strong>Exact destination (new folder only)</strong>
          <code className="writeTarget">{archiveReview.destinationDisplay}</code>
          <small>{archiveReview.files} files · {archiveReview.totalBytes} expanded bytes</small>
          <small>Entire tree manifest SHA-256: {archiveReview.manifestSha256}</small>
          <details open>
            <summary>Complete archive contents ({archiveReview.entries.length} entries)</summary>
            <pre className="editFullText" aria-label="All archive members">{archiveReview.entries.map((entry,i) =>
              (i + 1) + '. ' + entry.kind + ' · ' + entry.path +
              (entry.kind === 'file' ? ' · ' + entry.bytes + ' bytes · ' + entry.sha256 : '')
            ).join('\n')}</pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={archiveReviewed}
              onChange={e => setArchiveReviewed(e.target.checked)}/>
            I have reviewed the complete member list, destination, and no-overwrite behavior.
          </label>
          <small>Lu will ask again in a native confirmation dialog. If extraction is interrupted,
            partial files are retained for manual recovery and will never be silently retried.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!archiveReviewed || archiveBusy}
              onClick={() => void confirmArchiveTree()}>
              Approve in native confirmation
            </button>
            <button type="button" className="subtleButton" disabled={archiveBusy}
              onClick={() => void denyArchiveTree()}>Deny extraction</button>
            {archiveBusy && <button type="button" className="subtleButton"
              disabled={archiveCancelRequested}
              onClick={() => void requestArchiveCancellation()}>
              {archiveCancelRequested ? 'Cancellation requested' : 'Cancel running extraction'}
            </button>}
          </div>
        </section>}
        {latestArchiveExtract?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Archive extracted into {latestArchiveExtract.targetFolder}</strong>
          <code className="writeTarget">{latestArchiveExtract.exactDestination}</code>
          <small>{latestArchiveExtract.filesExtracted} verified files · {latestArchiveExtract.bytesExtracted} bytes</small>
          <small>Manifest SHA-256: {latestArchiveExtract.manifestSha256}</small>
          <small>Durable effect receipt: {latestArchiveExtract.effectCallId}</small>
        </section>}
        {Object.values(writeProposals).filter(p => p.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard writeProposalCard"
            role="group" aria-label="Review proposed new file">
            <strong>Lu proposes creating a NEW file (no overwrite)</strong>
            <p>{proposal.purpose}</p>
            <small>Filename: {proposal.fileName} · {proposal.bytes} bytes · SHA-256: {proposal.sha256}</small>
            <small>Model-selected text format: {proposal.fileFormat || 'UTF-8 text'}.
              JSON/JSONL receive syntax validation; source-code syntax needs its relevant parser or compiler.</small>
            <small>Review all proposed content before allowing creation:</small>
            <pre className="writePreview">{proposal.content}</pre>
            {proposal.destination ? <>
              <strong>Exact reviewed destination</strong>
              <code className="writeTarget">{proposal.destination.exactTarget}</code>
              <small>Only this destination and these exact bytes may be created.
                This approval is single-use and will not survive an interrupted attempt.</small>
            </> : <small>By default, save in your Documents folder. You can change the default under Settings → Advanced, or choose another folder for this file. Lu never chooses a destination without your approval.</small>}
            <div className="settingsActions">
              {!proposal.destination ? <>
                <button type="button" className="subtleButton"
                  onClick={() => reviewWriteDestination(proposal, 'user_documents')}>Use Documents folder</button>
                <button type="button" className="primaryButton"
                  onClick={() => reviewWriteDestination(proposal, 'choose_folder')}>Choose another folder</button>
              </> :
                <button type="button" className="primaryButton"
                  onClick={() => confirmWrite(proposal)}>Confirm create-new file</button>}
              <button type="button" className="subtleButton"
                onClick={() => denyWrite(proposal)}>Deny</button>
            </div>
          </section>
        )}

        {Object.values(editProposals).filter(p => p.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard editProposalCard"
            role="group" aria-label="Review proposed existing-file edit">
            <strong>Lu proposes editing an EXISTING text file</strong>
            <p>{proposal.purpose}</p>
            <small>Suggested filename: {proposal.fileName}. Lu cannot select the path;
              you must choose the matching existing file with a native picker.</small>
            {!proposal.selected ? <>
              <small>Proposed replacement (not approved):</small>
              <pre className="editFullText">{proposal.content}</pre>
            </> : <>
              <strong>Exact selected target</strong>
              <code className="writeTarget">{proposal.selected.exactTarget}</code>
              <small>Original: {proposal.selected.originalBytes} bytes, SHA-256: {proposal.selected.originalSha256}</small>
              <small>Replacement: {proposal.selected.replacementBytes} bytes, SHA-256: {proposal.selected.replacementSha256}</small>
              <small>Review fingerprint: {proposal.selected.approvalSha256} · Scope: {proposal.selected.risk}</small>
              <strong>Complete line-change view</strong>
              <small>Every unchanged line is retained; removed lines begin with −, added lines with +.
                Large change regions are not minimized. No content is truncated.</small>
              <pre className="editFullText editDiff" aria-label="Complete change lines">{fullEditChangeLines(proposal.selected.original,proposal.selected.replacement)}</pre>
              <strong>Complete original file</strong>
              <pre className="editFullText" aria-label="Complete original">{proposal.selected.original}</pre>
              <strong>Complete proposed replacement</strong>
              <pre className="editFullText" aria-label="Complete replacement">{proposal.selected.replacement}</pre>
              <label className="editReviewAck">
                <input type="checkbox" checked={Boolean(editReviewed[proposal.proposalId])}
                  onChange={e => setEditReviewed(prev => ({
                    ...prev,[proposal.proposalId]:e.target.checked
                  }))}/>
                I reviewed the complete original, changes, replacement and exact destination.
              </label>
              <small>Checking this box is only a UI acknowledgement; an independent native
                confirmation and host-authorized, journaled operation are still required.</small>
            </>}
            <div className="settingsActions">
              {!proposal.selected ?
                <button type="button" className="primaryButton"
                  onClick={() => reviewExistingFile(proposal)}>Choose existing file and compare</button> :
                <button type="button" className="primaryButton"
                  disabled={!editReviewed[proposal.proposalId]}
                  onClick={() => approveExistingEdit(proposal)}>Review native confirmation and edit</button>}
              <button type="button" className="subtleButton"
                onClick={() => denyExistingEdit(proposal)}>Deny edit</button>
            </div>
          </section>
        )}
        {latestEdited?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Edited {latestEdited.fileName}</strong>
          <small>Effect receipt: {latestEdited.effectCallId}</small>
          <small>Before: {latestEdited.originalSha256} · After: {latestEdited.replacementSha256}</small>
          <button className="folderLink" type="button"
            onClick={() => openEditedFolder(latestEdited.proposalId,latestEdited.chatId)}>
            Open containing folder
          </button>
        </section>}
        {latestCreated?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Created {latestCreated.fileName}</strong>
          <small>{latestCreated.exactTarget}</small>
          <button className="folderLink" type="button"
            onClick={() => openCreatedFolder(latestCreated.proposalId, latestCreated.chatId)}>
            Open containing folder
          </button>
        </section>}
        {writeRecovery.length > 0 && <section className="readProposalCard"
          aria-label="Write recovery records">
          <strong>Write recovery and audit</strong>
          <small>Read-only evidence from durable approval and effect journals.
            Uncertain operations are not automatically retried.</small>
          {writeRecovery.map(record =>
            <div key={record.proposalId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Approval: {record.approval} · Effect: {record.effect} · {record.bytes} bytes</small>
              <small>Observed target: {record.observedTarget} (observation only; not execution proof)</small>
              <small>Reconciliation: {record.assessment} — no automatic retry or reauthorization</small>
              {record.effect === 'Applied' && record.approval === 'Approved' &&
                <button type="button" className="folderLink"
                  onClick={() => openCreatedFolder(record.proposalId, chatId)}>
                  Open containing folder
                </button>}
              {record.exactTarget && <code className="writeTarget">{record.exactTarget}</code>}
              <small>SHA-256: {record.sha256}</small>
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshWriteRecovery().catch(error => setError(String(error)))}>
            Refresh recovery evidence
          </button>
        </section>}

        {editRecovery.length > 0 && <section className="readProposalCard"
          aria-label="Existing file edit recovery records">
          <strong>Existing-file edit recovery</strong>
          <small>Read-only approval and effect journal evidence. File bytes are inspected only
            after you reselect the originally approved containing folder through the native picker.
            Uncertain edits never resume automatically.</small>
          {editRecovery.map(record =>
            <div key={record.proposalId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Review: {record.approval} · Effect: {record.effect}</small>
              <small>Exact applied edit receipt: {record.receiptVerified ? 'Verified' : 'Not verified'}</small>
              {record.exactTarget && <code className="writeTarget">{record.exactTarget}</code>}
              <small>Original SHA-256: {record.originalSha256 || 'Not selected'}</small>
              <small>Replacement SHA-256: {record.replacementSha256}</small>
              <small>Assessment: {(editObservations[record.proposalId]?.assessment || record.assessment)}</small>
              <small>Observed bytes: {(editObservations[record.proposalId]?.observation || record.observation)}
                (matching bytes alone do not prove who published them)</small>
              {editObservations[record.proposalId] && <small>
                Undo preflight: {editObservations[record.proposalId].rollbackReviewStatus}
              </small>}
              {editObservations[record.proposalId]?.rollbackPreview && <details
                className="editUndoPreview" aria-label="Read-only undo preview">
                <summary>Review verified original for a possible future restore (read-only)</summary>
                <small>No restore command is issued. This preview requires the exact
                  decoded APPLIED edit receipt, retained original backup and current file.</small>
                <small>Edited SHA-256: {editObservations[record.proposalId].rollbackPreview.replacementSha256}</small>
                <small>Original SHA-256: {editObservations[record.proposalId].rollbackPreview.originalSha256}</small>
                <strong>Complete changes if original were restored</strong>
                <pre className="editFullText editDiff" aria-label="Proposed restore change lines">{
                  fullEditChangeLines(editObservations[record.proposalId].rollbackPreview.currentText,
                    editObservations[record.proposalId].rollbackPreview.restoreText)
                }</pre>
                <strong>Complete current text</strong>
                <pre className="editFullText" aria-label="Current text for possible restore">{
                  editObservations[record.proposalId].rollbackPreview.currentText
                }</pre>
                <strong>Complete verified original text</strong>
                <pre className="editFullText" aria-label="Verified original text">{
                  editObservations[record.proposalId].rollbackPreview.restoreText
                }</pre>
                <small>This restoration has its own authorization. A native confirmation
                  will ask again before any filesystem operation. The edited version is separately
                  backed up. Changes made by another program will block the restore.</small>
                {editObservations[record.proposalId].rollbackReviewId && <>
                  <label className="editReviewAck">
                    <input type="checkbox"
                      checked={Boolean(rollbackReviewed[editObservations[record.proposalId].rollbackReviewId])}
                      onChange={e => setRollbackReviewed(prev => ({
                        ...prev,
                        [editObservations[record.proposalId].rollbackReviewId]:e.target.checked
                      }))}/>
                    I reviewed the complete current file, original version and restoration changes.
                  </label>
                  <div className="settingsActions">
                    <button className="primaryButton" type="button"
                      disabled={!rollbackReviewed[editObservations[record.proposalId].rollbackReviewId]}
                      onClick={() => confirmRollback(record)}>
                      Confirm original restore in native dialog
                    </button>
                    <button className="subtleButton" type="button"
                      onClick={() => denyRollback(record)}>Keep current version</button>
                  </div>
                </>}
              </details>}
              {record.approval === 'Approved' && <button type="button" className="subtleButton"
                onClick={() => inspectEditRecovery(record)}>Reselect folder to inspect artifacts</button>}
              {record.approval === 'Approved' && record.receiptVerified &&
                <button type="button" className="folderLink"
                  onClick={() => openEditedFolder(record.proposalId,chatId)}>
                  Open containing folder
                </button>}
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshEditRecovery().catch(error => setError(String(error)))}>
            Refresh edit recovery ledger
          </button>
        </section>}
        {rollbackHistory.length > 0 && <section className="readProposalCard"
          aria-label="Restoration authorization and recovery history">
          <strong>Restoration history and recovery</strong>
          <small>Separate user approval, journal effect, and verified receipt. A failed or
            uncertain restore is never automatically repeated, even after a restart.</small>
          {rollbackHistory.map(record =>
            <div key={record.reviewId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Review: {record.decision} · Restore effect: {record.effect}</small>
              <small>Verified restoration receipt: {record.receiptVerified ? 'Yes' : 'No'}</small>
              <small>Assessment: {record.assessment}</small>
              <small>Original edit effect: {record.originalEditId}</small>
              <small>Restore SHA-256: {record.restoredSha256}</small>
              <code className="writeTarget">{record.exactTarget}</code>
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshRollbackHistory().catch(error => setError(String(error)))}>
            Refresh restoration history
          </button>
        </section>}
        {latestRestored?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Original version restored: {latestRestored.fileName}</strong>
          <small>Restored {latestRestored.restoredBytes} bytes · SHA-256: {latestRestored.restoredSha256}</small>
          <small>New rollback effect: {latestRestored.rollbackEffectId}</small>
          <small>The prior edited bytes were retained as a separate rollback backup.</small>
        </section>}
        {!composerCollapsed && <form className={'composer' + (dragActive ? ' dragActive' : '')} onSubmit={send}>
          {(images.length > 0 || textFiles.length > 0) && <div className="attachments" aria-label="Draft attachments">
            {images.map(im => <span key={im.imageRef} className="attachment imageAttachment">
              {im.previewUrl ? <img src={im.previewUrl} alt="Attached image preview"/> : <span aria-hidden="true">▧</span>}
              <span title={im.name || im.mimeType}>{im.name || im.mimeType}{im.detectedMimeType && im.name?.toLowerCase().endsWith('.jpg') && im.detectedMimeType === 'image/png' ? ' (PNG data)' : ''}</span>
              {im.previewUrl && <button type="button" onClick={()=>setImageViewer({imageRef:im.imageRef,zoom:1})} aria-label={'View image '+(im.name||'draft')}>View</button>}
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
            <button type="button" className="subtleButton" disabled={!loaded}
              title="Inspect native vision status without claiming MMProj presence proves visual understanding"
              onClick={()=>void invoke('lu_vision_capability')
                .then(setVisionCapability).catch(e=>setError(String(e)))}>Check vision</button>
            <button type="button" className="subtleButton"
              disabled={!chatId||!loaded||visionProbeBusy||working}
              title="Native approval; submit private synthetic pixel images to the actually loaded GGUF/MMProj backend"
              onClick={()=>void verifyNativeVision()}>
              {visionProbeBusy?'Running pixel probe…':'Verify vision with real images'}
            </button>
            {visionCapability?.sessionId===runtime.sessionId && visionCapability?.modelRef===runtime.modelRef &&
              <span className="composerHelp" role="status">
                Vision: {visionCapability.status} · {visionCapability.detail}
              </span>}
            <button type="button" className="subtleButton"
              disabled={!chatId || wave2RecoveryBusy}
              title="Read-only durable history of approved archives, documents and file operations"
              onClick={()=>void refreshWave2Recovery(chatId)}>
              {wave2RecoveryBusy ? 'Checking operations…' : 'File operations'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || trashBusy || Boolean(trashReview) || Boolean(restoreReview)}
              title="Native-pick one file and review a reversible same-directory custody operation"
              onClick={()=>void reviewNativeTrashFile()}>Trash file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || trashBusy} title="Inspect journaled trash/restore receipts, including interrupted operations"
              onClick={()=>{setTrashHistoryOpen(v=>!v);void refreshTrashHistory();}}>
              File recovery
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || fileTransferBusy || Boolean(fileTransferReview)}
              title="Native-pick an existing file, review exact SHA-256 and approve a no-clobber copy"
              onClick={()=>void reviewNativeFileTransfer('copy')}>Copy file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || fileTransferBusy || Boolean(fileTransferReview)}
              title="Native-pick an existing file and approve a same-directory move into a new filename"
              onClick={()=>void reviewNativeFileTransfer('move')}>Move file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || officeBatchBusy || Boolean(officeBatchReview)}
              title="Review up to 32 Word, PowerPoint or Excel changes, then create one new version"
              onClick={()=>setOfficeBatchOpen(v=>!v)}>
              {officeBatchOpen ? 'Close batch' : 'Batch Office'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || officeRevisionBusy || Boolean(officeRevisionReview)}
              title="Revise one existing Office field while preserving original and unrelated package contents"
              onClick={()=>setOfficeRevisionOpen(v=>!v)}>
              {officeRevisionOpen ? 'Close revision' : 'Revise Office'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || documentCreateBusy || Boolean(documentCreateReview)}
              title="Create a new DOCX or PPTX through a native folder picker and exact-content approval"
              onClick={() => setDocumentComposerOpen(v=>!v)}>
              {documentComposerOpen ? 'Close document' : '+ Document'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || archiveBusy || Boolean(archiveReview)}
              title="Use native file and folder pickers to safely review an archive and extract it into one NEW folder"
              onClick={() => void reviewArchiveTree()}>{archiveBusy ? 'Archive…' : 'Extract archive'}</button>
            <button type="button" className="subtleButton"
              disabled={!chatId||working||chatBusy||archiveBusy||archiveBatchBusy
                ||Boolean(archiveReview)||Boolean(archiveBatchReview)}
              title="Pick 1–8 archives in a native dialog; inspect every entry and approve one NEW group"
              onClick={()=>void reviewArchiveBatch()}>
              {archiveBatchBusy?'Reviewing batch…':'Extract archive batch'}
            </button>
            <label className="readerSectionSelector"
              title="Lu archive byte-part size, not spanned ZIP/RAR/7z format">
              Lu multipart size
              <select aria-label="Lu multipart archive part size MiB"
                value={p08PartMib} disabled={p08Busy}
                onChange={e=>setP08PartMib(Number(e.target.value))}>
                {[4,8,16,32].map(x=><option key={x} value={x}>{x} MiB</option>)}
              </select>
            </label>
            <button type="button" className="subtleButton"
              disabled={!chatId||working||chatBusy||p08Busy||archiveBusy||archiveBatchBusy}
              title="OS-pick existing archive, review SHA-256, split to NEW Lu part folder"
              onClick={()=>void runP08Multipart('split')}>
              {p08Busy?'Multipart operation…':'Split archive into Lu parts'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId||working||chatBusy||p08Busy}
              title="OS-pick interrupted Lu stage folder and rehash source and existing parts"
              onClick={()=>void runP08Multipart('resume')}>Resume Lu split stage</button>
            <button type="button" className="subtleButton"
              disabled={!chatId||working||chatBusy||p08Busy}
              title="OS-pick lu-parts.json plus destination; verify all SHA-256 and create only a NEW archive"
              onClick={()=>void runP08Multipart('join')}>Reassemble Lu archive parts</button>
            <button type="button" className="subtleButton"
              disabled={!chatId||working||chatBusy||p08Busy}
              title="OS-pick original Lu split stage; inspect partial output without writing, replaying or deleting"
              onClick={()=>void runP08Multipart('review')}>Review interrupted Lu split (read-only)</button>
            {p08ResultByChat[chatId]?.status&&
              <div className="researchResults" role="status" aria-label="P08 recovery review">
                <strong>{p08ResultByChat[chatId].status}: {p08ResultByChat[chatId].outputFolder}</strong>
                <small>{p08ResultByChat[chatId].verifiedParts} / {p08ResultByChat[chatId].expectedParts} verified parts</small>
                <small>{p08ResultByChat[chatId].note}</small>
              </div>}
            {p08ResultByChat[chatId]&&!p08ResultByChat[chatId].status&&
              <div className="researchResults" role="status" aria-label="P08 multipart archive receipt">
                <strong>{p08ResultByChat[chatId].destination}</strong>
                <small>{p08ResultByChat[chatId].parts} parts · {p08ResultByChat[chatId].bytes} bytes
                  · original SHA-256: {p08ResultByChat[chatId].sha256}</small>
                <small>Manifest SHA-256: {p08ResultByChat[chatId].manifestSha256}</small>
                <small>{p08ResultByChat[chatId].note}</small>
                <small>Stages are retained for reconciliation. Lu parts are not natively
                  spanned ZIP/RAR/7z; archives are not converted or repaired.</small>
              </div>}
            {archiveBatchBusy&&!archiveBatchReview&&
              <button type="button" className="subtleButton"
                disabled={archiveBatchCancelRequested}
                onClick={()=>void cancelArchiveBatch()}>
                {archiveBatchCancelRequested?'Stopping batch preview…':'Cancel batch preview'}
              </button>}
            {archiveBusy && !archiveReview && <button type="button"
              className="subtleButton" disabled={archiveCancelRequested}
              onClick={()=>void cancelArchivePreview()}>
              {archiveCancelRequested?'Stopping archive preview…':'Cancel archive preview'}
            </button>}
            <button type="button" className="subtleButton" disabled={working || chatBusy} onClick={inspectSelectedTextFile} title="Choose a local text file in a native file picker and inspect it using Lu’s explicitly granted read-only native tool">Inspect file</button>
            <label className="readerSectionSelector" title="Use Lu's existing safe Reader selectors">
              Reader section
              <input type="text" maxLength={160}
                placeholder="page:2 · chapter:3 · notes:1 · range:Sales!A1:D10"
                value={readerSelector}
                onChange={e=>{
                  setReaderSelector(e.target.value);
                  setReaderContinuation(null);
                }}/>
            </label>
            <button type="button" className="subtleButton" disabled={!loaded}
              onClick={()=>void attachReaderDocument(false)}>+ Read file</button>
            {readerContinuation?.chatId===(currentChat.current||'new')&&
              readerContinuation.selector===readerSelector&&
              readerContinuation.nextOffset!==null&&
              <button type="button" className="subtleButton" disabled={!loaded}
                title={'Reselect '+readerContinuation.name+
                  ' to verify its SHA-256 before reading the next excerpt'}
                onClick={()=>void attachReaderDocument(true)}>
                Continue Reader at {readerContinuation.nextOffset}
              </button>}
            <button type="button" className="subtleButton"
              disabled={!loaded||chatBusy||working||!prompt.trim()}
              title="Select one document and allow a read-only AI Reader tool turn"
              onClick={()=>void askReaderDocument()}>Ask Reader</button>
            <button type="button" className="subtleButton"
              disabled={!loaded||chatBusy||working||!prompt.trim()}
              title="Choose a folder and let Lu discover files by name, type, size, date or bounded text, then inspect or read them under one approved read-only tool turn"
              onClick={()=>void askWorkspaceDirectory()}>Ask Workspace</button>
            <button type="button" className="subtleButton"
              aria-pressed={workspaceToolsNextSend}
              disabled={!loaded||chatBusy||working||Boolean(images.length)||Boolean(textFiles.length)}
              title="On the next normal Send, choose a folder in the native picker and let Lu use approved read-only tools for that one turn"
              onClick={()=>{setBrowserResearchNextSend(false);setWorkspaceToolsNextSend(value=>!value);}}>
              {workspaceToolsNextSend?'Workspace tools: next Send ✓':'Use workspace tools on Send'}
            </button>
            {workspaceToolsNextSend && <span className="composerHelp" role="status">
              Next Send will open a native folder picker. Read-only access is granted for that turn only.
            </span>}
            <button type="button" className="subtleButton"
              aria-pressed={browserResearchNextSend}
              disabled={!loaded||chatBusy||working||Boolean(images.length)||Boolean(textFiles.length)}
              title="Next Send: request native approval for exact HTTPS origins, use a supervised read-only Browser and return a journaled answer"
              onClick={()=>{setWorkspaceToolsNextSend(false);setBrowserResearchNextSend(v=>!v);}}>
              {browserResearchNextSend?'Browser research: next Send ✓':'Research with Lu browser (read only)'}
            </button>
            {browserResearchNextSend&&<label className="composerHelp">
              Research HTTPS origins (exact public sites)
              <input value={browserOrigins} maxLength={2048}
                onChange={e=>setBrowserOrigins(e.target.value)}
                placeholder="https://example.org" />
              <small>Lu asks for native approval of the entire list.
                Text-only, read-only, one conversation turn.</small>
            </label>}
            <button type="button" className="subtleButton"
              disabled={!chatId}
              title="Inspect actual persisted Reader/Workspace model tool loops without replaying effects"
              onClick={()=>{setToolActivityOpen(v=>!v);void refreshToolActivity(chatId);}}>
              {toolActivityOpen?'Hide tool activity':'Tool activity'}
            </button>
            <span className="composerHelp">Explicit workspace searches and document-reading requests open native pickers; Git status, log and diff require selecting a repository · No automatic file permission</span>
            <span className="composerHelp">Paste or drop images and text · Enter to send · Shift+Enter for newline</span>
            {chatBusy ? <button className="dangerButton" type="button" onClick={cancel}
                disabled={!visibleJob.generationId || visibleJob.cancelling}>{visibleJob.cancelling ? 'Stopping…' : visibleJob.phase === 'queued' ? 'Cancel queued' : 'Cancel'}</button>
              : <button className="primaryButton" type="submit" disabled={working || !loaded || !prompt.trim()}>Send</button>}
          </div></form>}
      </div>
    </main>
    {viewedDraftImage?.previewUrl && <div className="imageViewerBackdrop" role="presentation"
      onClick={()=>setImageViewer(null)}>
      <section className="imageViewerDialog" role="dialog" aria-modal="true"
        aria-label="Image Workspace preview" onClick={e=>e.stopPropagation()}>
        <header>
          <strong>Image Workspace · original and immutable revision export</strong>
          <button type="button" aria-label="Close image preview"
            onClick={()=>setImageViewer(null)}>×</button>
        </header>
        <small>{viewedDraftImage.name||'Attached image'} · {viewedDraftImage.detectedMimeType||viewedDraftImage.mimeType||'Verified image'}</small>
        <div className="imageViewerViewport" aria-label="Zoomable original draft image">
          <div style={{display:'inline-block',position:'relative',
            transform:'scale('+imageViewer.zoom+')',transformOrigin:'top left'}}>
            <img src={viewedDraftImage.previewUrl}
              alt="Original attached image, drag to select pixels"
              onPointerDown={selectImagePointerStart}
              onPointerMove={selectImagePointerMove}
              onPointerUp={selectImagePointerEnd}
              onPointerCancel={()=>{imageDragRef.current=null;}}
              onLoad={e=>{
                const width=e.currentTarget.naturalWidth,height=e.currentTarget.naturalHeight;
                setImageViewer(v=>v&&v.imageRef===viewedDraftImage.imageRef
                  ?({...v,width,height}):v);
                setImageEdit({
                  mode:'resize',x:'0',y:'0',width:String(width),height:String(height),
                  blockSize:'12',stroke:'3',color:'#ff5500',
                  amount:'0',sigma:'2',threshold:'3'
                });
                setImageSelection(null);setImageExportReceipt(null);
              }}
              style={{touchAction:'none',userSelect:'none',cursor:'crosshair'}}/>
            {imageSelection?.chatId===chatId&&
              imageSelection.imageRef===imageViewer.imageRef&&
              <div aria-hidden="true" style={{
                position:'absolute',pointerEvents:'none',boxSizing:'border-box',
                left:(100*imageSelection.x/imageViewer.width)+'%',
                top:(100*imageSelection.y/imageViewer.height)+'%',
                width:(100*imageSelection.width/imageViewer.width)+'%',
                height:(100*imageSelection.height/imageViewer.height)+'%',
                border:'2px solid #4b9cff',background:'rgba(75,156,255,0.09)'
              }}/>}
          </div>
        </div>
        <small>Drag on the original image to select an OCR rectangle.
          Before staging edits, the same rectangle fills crop/mask/annotation
          dimensions. For later assembly steps, enter coordinates in the
          current staged coordinate space.</small>
        <footer>
          <button type="button" disabled={imageViewer.zoom<=0.5}
            onClick={()=>setImageViewer(v=>v&&({...v,zoom:Math.max(0.5,+(v.zoom-0.25).toFixed(2))}))}>Zoom out</button>
          <span>{Math.round(imageViewer.zoom*100)}%</span>
          <button type="button" disabled={imageViewer.zoom>=3}
            onClick={()=>setImageViewer(v=>v&&({...v,zoom:Math.min(3,+(v.zoom+0.25).toFixed(2))}))}>Zoom in</button>
          <button type="button" onClick={()=>setImageViewer(null)}>Close</button>
        </footer>
        <div className="imageRevisionForm" aria-label="Offline image text extraction">
          <strong>Offline OCR — review only</strong>
          <small>Select a PNG/JPEG using the native file picker. Lu will use only an
            approved bundled OCR runtime; no images are sent to a web service.
            Recognized text may be inaccurate and is never treated as a verified passage.</small>
          <button type="button" onClick={()=>void readAttachedImageText()}
            disabled={ocrBusy||!chatId||!imageViewer?.imageRef}>
            {ocrBusy?'Offline OCR active…':'Recognize text in this attached image'}
          </button>
          {imageViewer?.imageRef&&<div aria-label="Original image OCR rectangle">
            <small>Read only this original-image rectangle (not staged-edit coordinates).
              Leave width/height blank to reach the image edge.
              Original: {imageViewer.width} × {imageViewer.height} pixels.</small>
            <div className="imageRevisionDimensions">
              {['x','y','width','height'].map(field=><label key={field}>
                OCR {field}<input type="number" min={field==='x'||field==='y'?0:1}
                  max={8192} value={ocrRegion[field]}
                  onChange={e=>setOcrRegion(v=>({...v,[field]:e.target.value}))}/>
              </label>)}
            </div>
            <button type="button" disabled={ocrBusy||!chatId}
              onClick={()=>void readAttachedRegionText()}>
              Read selected original region
            </button>
          </div>}
          <button type="button" onClick={()=>void readOfflineImageText()}
            disabled={ocrBusy||!chatId}>
            {ocrBusy?'Recognizing from selected image…':'Choose image and extract text'}
          </button>
          <div className="imageRevisionDimensions" aria-label="Offline scanned PDF page OCR">
            <label>First PDF page (1–32)
              <input type="number" min="1" max="32" value={ocrPdfPage}
                disabled={ocrBusy}
                onChange={e=>{setOcrPdfPage(e.target.value);
                  setOcrPdfEndPage(e.target.value);}}/>
            </label>
            <label>Last PDF page (up to 8 at a time)
              <input type="number" min="1" max="32" value={ocrPdfEndPage}
                disabled={ocrBusy}
                onChange={e=>setOcrPdfEndPage(e.target.value)}/>
            </label>
            <button type="button" disabled={!chatId||ocrBusy}
              onClick={()=>void readSelectedPdfPage()}>
              Choose PDF and OCR selected pages
            </button>
          </div>
          <small>PDF OCR requires separately licensed and hash-pinned offline
            rasterizer resources in the installed application. Text and
            rendered pixels remain unverified, even when extraction succeeds.</small>
          {ocrBusy&&<button type="button" onClick={()=>void cancelOfflineOcr()}>
            Stop active OCR worker
          </button>}
          {ocrObservation?.chatId===chatId&&
            (!ocrObservation.imageRef||ocrObservation.imageRef===imageViewer?.imageRef)&&<div>
            {ocrPdfBatch.length>1&&ocrPdfBatch[0].chatId===chatId&&
              <label>Review OCR page
                <select value={ocrPdfIndex}
                  onChange={e=>selectOcrPdfPage(Number(e.target.value))}>
                  {ocrPdfBatch.map((result,index)=><option
                    key={result.page} value={index}>Page {result.page}
                  </option>)}
                </select>
              </label>}
            <small>Unverified OCR · source SHA-256 {ocrObservation.sourceSha256}
              · text SHA-256 {ocrObservation.textSha256}</small>
            {ocrObservation.page&&<small>
              OCR page {ocrObservation.page} · Derived raster SHA-256
              {' '}{ocrObservation.ocrInputSha256} · renderer SHA-256
              {' '}{ocrObservation.rasterEngineSha256}
            </small>}
            {ocrObservation.region&&<small>
              Original image OCR region:
              ({ocrObservation.region.x},{ocrObservation.region.y}),
              {ocrObservation.region.width} × {ocrObservation.region.height} px;
              derived PNG SHA-256 {ocrObservation.ocrInputSha256}
            </small>}
            <small>Editable OCR review, maximum 12,000 characters.
              The worker text SHA-256 identifies the uncorrected transcript,
              not any edits made here.
              {ocrObservation.text.length>12_000&&
                ' Longer OCR output was clipped for this draft review.'}</small>
            <textarea rows={5} maxLength={12_000}
              aria-label="Review and correct unverified OCR transcription"
              value={ocrReviewText}
              onChange={e=>{
                setOcrReviewText(e.target.value);
                if(ocrPdfBatch.length&&ocrObservation?.page){
                  setOcrPdfEdits(v=>({...v,[ocrObservation.page]:e.target.value}));
                }
              }}/>
            <button type="button" onClick={()=>{
              if(ocrObservation?.chatId!==chatId||
                (ocrObservation.imageRef&&ocrObservation.imageRef!==imageViewer?.imageRef))return;
              const edited=ocrReviewText.trim();
              if(!edited){setError('Review some OCR text before inserting it.');return;}
              const candidate=prompt+(prompt?'\n\n':'')+
                '[User-edited OCR transcription — unverified image text]\n'+edited;
              if(candidate.length>16*1024){
                setError('The OCR excerpt exceeds Lu\'s 16 KiB chat draft limit. Shorten it.');
                return;
              }
              setPrompt(candidate);
              // Each PDF page must be reviewed before insertion. Do not
              // silently concatenate unchecked pages into a prompt.
              if(ocrPdfBatch.length>1&&ocrPdfIndex+1<ocrPdfBatch.length){
                selectOcrPdfPage(ocrPdfIndex+1);
              }else{
                setOcrObservation(null);setOcrReviewText('');
                setOcrPdfBatch([]);setOcrPdfEdits({});
              }
            }}>Insert reviewed OCR text into draft</button>
          </div>}
        </div>
        <div className="imageRevisionForm" aria-label="Deterministic image revision export">
          <small>Original stays unchanged. Native approval and a NEW PNG destination are required.
            Solid masks remove pixels in the exported file; mosaics are visual effects, not secure redaction.
            AI-generated editing and automatic source overwrite remain disabled.</small>
          <label>Operation
            <select value={imageEdit.mode} disabled={imageExportBusy}
              onChange={e=>setImageEdit(v=>({...v,mode:e.target.value}))}>
              <option value="resize">Resize</option>
              <option value="crop">Crop</option>
              <option value="rotate90">Rotate 90°</option>
              <option value="flip_h">Flip horizontally</option>
              <option value="redact">Solid privacy mask</option>
              <option value="pixelate">Pixelate selected region</option>
              <option value="rectangle">Rectangle outline</option>
              <option value="line">Diagonal line annotation</option>
              <option value="ellipse">Ellipse outline annotation</option>
              <option value="arrow">Arrow annotation</option>
              <option value="grayscale">Grayscale (preserve alpha)</option>
              <option value="invert">Invert RGB (preserve alpha)</option>
              <option value="blur">Gaussian blur</option>
              <option value="brightness">Brightness adjustment</option>
              <option value="contrast">Contrast adjustment</option>
              <option value="sharpen">Sharpen</option>
            </select>
          </label>
          {REGION_IMAGE_OPERATIONS.includes(imageEdit.mode)&&<div className="imageRevisionDimensions">
            <label>X <input type="number" min="0" value={imageEdit.x}
              onChange={e=>setImageEdit(v=>({...v,x:e.target.value}))}/></label>
            <label>Y <input type="number" min="0" value={imageEdit.y}
              onChange={e=>setImageEdit(v=>({...v,y:e.target.value}))}/></label>
          </div>}
          {!['rotate90','flip_h','grayscale','invert','blur',
             'brightness','contrast','sharpen'].includes(imageEdit.mode)&&
             <div className="imageRevisionDimensions">
            <label>Width <input type="number" min="1" max="8192" value={imageEdit.width}
              onChange={e=>setImageEdit(v=>({...v,width:e.target.value}))}/></label>
            <label>Height <input type="number" min="1" max="8192" value={imageEdit.height}
              onChange={e=>setImageEdit(v=>({...v,height:e.target.value}))}/></label>
          </div>}
          {['brightness','contrast'].includes(imageEdit.mode)&&<label>
            {imageEdit.mode==='brightness'?'Brightness (-128 to 128)':'Contrast (-100 to 100)'}
            <input type="number" min={imageEdit.mode==='brightness'?-128:-100}
              max={imageEdit.mode==='brightness'?128:100}
              value={imageEdit.amount}
              onChange={e=>setImageEdit(v=>({...v,amount:e.target.value}))}/>
          </label>}
          {['blur','sharpen'].includes(imageEdit.mode)&&<label>Filter radius (1–8)
            <input type="number" min="1" max="8" value={imageEdit.sigma}
              onChange={e=>setImageEdit(v=>({...v,sigma:e.target.value}))}/>
          </label>}
          {imageEdit.mode==='sharpen'&&<label>Sharpen threshold (0–64)
            <input type="number" min="0" max="64" value={imageEdit.threshold}
              onChange={e=>setImageEdit(v=>({...v,threshold:e.target.value}))}/>
          </label>}
          {imageEdit.mode==='pixelate'&&<label>Mosaic block size
            <input type="number" min="2" max="128" value={imageEdit.blockSize}
              onChange={e=>setImageEdit(v=>({...v,blockSize:e.target.value}))}/>
          </label>}
          {['rectangle','line','ellipse','arrow'].includes(imageEdit.mode)&&<div className="imageRevisionDimensions">
            <label>Annotation color <input type="color" value={imageEdit.color}
              onChange={e=>setImageEdit(v=>({...v,color:e.target.value}))}/></label>
            <label>Stroke width <input type="number" min="1"
              max={['line','arrow'].includes(imageEdit.mode)?32:64} value={imageEdit.stroke}
              onChange={e=>setImageEdit(v=>({...v,stroke:e.target.value}))}/></label>
          </div>}
          <div className="imageRevisionDimensions">
            <button type="button" disabled={imageExportBusy||!chatId}
              onClick={()=>queueImageStep()}>Add step to edit assembly</button>
            <button type="button" disabled={imageExportBusy||imagePlan.chatId!==chatId||
              imagePlan.imageRef!==imageViewer.imageRef||!imagePlan.steps.length}
              onClick={()=>undoImageStep()}>Undo step</button>
            <button type="button" disabled={imageExportBusy||imagePlan.chatId!==chatId||
              imagePlan.imageRef!==imageViewer.imageRef||!imagePlan.redo.length}
              onClick={()=>redoImageStep()}>Redo step</button>
            <button type="button" disabled={imageExportBusy}
              onClick={()=>setImagePlan({chatId,imageRef:imageViewer.imageRef,steps:[],redo:[]})}>
              Clear assembly
            </button>
          </div>
          {imagePlan.chatId===chatId&&imagePlan.imageRef===imageViewer.imageRef&&
            imagePlan.steps.length>0&&<div aria-label="Staged immutable image edits">
              <small>{imagePlan.steps.length} ordered operations ·
                staged dimensions {stagedImageDimensions(
                  imageViewer.width,imageViewer.height,imagePlan.steps)?.width} ×
                {stagedImageDimensions(
                  imageViewer.width,imageViewer.height,imagePlan.steps)?.height}.
                Undo/redo changes only the assembly, never the source.</small>
              <ol>{imagePlan.steps.map((step,i)=><li key={i}>
                {step.operation}{['crop','redact','pixelate','rectangle','line','ellipse','resize'].includes(step.operation)?
                  ' · '+step.width+' × '+step.height:''}
              </li>)}</ol>
            </div>}
          <button type="button" disabled={!chatId||imageExportBusy||imagePreviewBusy||
              imagePlan.chatId!==chatId||imagePlan.imageRef!==imageViewer.imageRef||
              !imagePlan.steps.length}
            onClick={()=>void previewImageAssembly()}>
            {imagePreviewBusy?'Preparing native preview…':'Preview staged edits'}
          </button>
          {imageAssemblyPreview?.key===currentImagePreviewKey&&
            imageAssemblyPreview.result?.dataUrl&&
            <div className="imageViewerViewport" aria-label="Native staged image preview">
              <div style={{position:'relative',display:'inline-block'}}>
                <img src={imageAssemblyPreview.result.dataUrl}
                  alt="Preview of staged edits; drag to select the next edit rectangle"
                  onPointerDown={startStagedSelection}
                  onPointerMove={moveStagedSelection}
                  onPointerUp={endStagedSelection}
                  onPointerCancel={()=>{stageDragRef.current=null;}}
                  style={{display:'block',touchAction:'none',cursor:'crosshair',
                    userSelect:'none'}}/>
                {stageSelection?.key===currentImagePreviewKey&&
                  <div aria-hidden="true" style={{
                    position:'absolute',pointerEvents:'none',boxSizing:'border-box',
                    left:(100*stageSelection.x/imageAssemblyPreview.result.width)+'%',
                    top:(100*stageSelection.y/imageAssemblyPreview.result.height)+'%',
                    width:(100*stageSelection.width/imageAssemblyPreview.result.width)+'%',
                    height:(100*stageSelection.height/imageAssemblyPreview.result.height)+'%',
                    border:'2px solid #4b9cff',background:'rgba(75,156,255,0.09)'
                  }}/>}
              </div>
              <small>Volatile native preview ·
                {imageAssemblyPreview.result.width} × {imageAssemblyPreview.result.height} ·
                SHA-256 {imageAssemblyPreview.result.revisionSha256}.
                Drag to select coordinates for your next staged edit.
                No file saved. Source image remains unchanged.</small>
            </div>}
          <button type="button" disabled={!chatId||!imageViewer?.width||imageExportBusy}
            onClick={()=>void exportImageRevision()}>
            {imageExportBusy?'Awaiting native approval…':'Save new PNG revision…'}
          </button>
          {imageExportReceipt&&<small role="status">
            New image version saved: {imageExportReceipt.fileName} ·
            {imageExportReceipt.width} × {imageExportReceipt.height} ·
            SHA-256 {imageExportReceipt.revisionSha256}. Original retained.
          </small>}
          <button type="button" disabled={!chatId||imageHistoryBusy}
            onClick={()=>void loadImageRevisionHistory()}>
            {imageHistoryBusy?'Reading history…':'Review saved revision history'}
          </button>
          {imageHistoryView.chatId===chatId&&
            <div aria-label="Historical image revision receipts">
              <small>Historical exports only. This list does not prove files still exist.
                Uncertain exports are never automatically retried.</small>
              <ul>{imageHistoryView.rows.map(record=><li key={record.id}>
                {record.fileName} · {record.width} × {record.height} ·
                {record.state} · SHA-256 {record.revisionSha256}
                {record.parentRevisionId&&<small> · Derived from earlier export
                  {' '}{record.parentRevisionId}</small>}
              </li>)}</ul>
            </div>}
        </div>
      </section>
    </div>}
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
        <div className="backgroundContinuationPreference">
          <label>
            <input type="checkbox" disabled={!chatId}
              checked={Boolean(chatId&&autoBackgroundContinuation)}
              onChange={e=>void changeAutoBackgroundContinuation(e.target.checked)}/>
            Automatically notify this conversation's AI when a task ends
          </label>
          <small>Off by default. When enabled, Lu may start a short AI reply after completion, failure, cancellation, interruption, or timeout. Does not grant tool permissions or load a model.</small>
        </div>
        <div className="backgroundScheduleOptions" aria-label="Background scan scheduling">
          <label>When
            <select value={backgroundDelay} onChange={e=>setBackgroundDelay(e.target.value)}>
              <option value="0">As resources permit</option>
              <option value="5">In 5 minutes</option>
              <option value="30">In 30 minutes</option>
              <option value="60">In 1 hour</option>
              <option value="1440">Tomorrow</option>
            </select>
          </label>
          <label>Repeat
            <select value={backgroundRepeat} onChange={e=>setBackgroundRepeat(e.target.value)}>
              <option value="0">Once</option>
              <option value="60">Every hour</option>
              <option value="1440">Daily</option>
              <option value="10080">Weekly</option>
            </select>
          </label>
          <label>Priority
            <select value={backgroundPriority} onChange={e=>setBackgroundPriority(e.target.value)}>
              <option value="background">Low</option>
              <option value="normal">Normal</option>
              <option value="interactive">High (still resource-limited)</option>
            </select>
          </label>
        </div>
        {backgroundGuardian && <div className="backgroundGuardianStatus" role="status">
          <strong>Resource Guardian</strong>
          <span>Queued: {backgroundGuardian.queued} · Running: {backgroundGuardian.running} · Paused: {backgroundGuardian.paused}</span>
          <span>Workers: {backgroundGuardian.workerUsed}/{backgroundGuardian.workerCapacity} · CPU slots: {backgroundGuardian.cpuUsed}/{backgroundGuardian.cpuCapacity} · RAM reservation: {backgroundGuardian.ramUsedMib}/{backgroundGuardian.ramCapacityMib} MiB</span>
          <small>{backgroundGuardian.observedAvailableRamMib==null?'Physical RAM probe unavailable':('Physical RAM available: '+backgroundGuardian.observedAvailableRamMib+' MiB')}{backgroundGuardian.observedCpuBusyPercent==null?'':' · CPU busy '+backgroundGuardian.observedCpuBusyPercent+'%'}</small>
          <small>GPU tasks stay disabled without verified backend VRAM availability. CPU/RAM admission reacts to host pressure; reservations are not actual per-worker consumption.</small>
        </div>}
        <div className="backgroundScanLaunch">
          <button type="button" className="primaryButton"
            onClick={()=>void startMediaFileScan()}>
            Scan media file…
          </button>
          <small>Native SHA-256 integrity scan · queued by Lu. Recurring scans require new authorization after app restart.</small>
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
            {task.priority && <small>Priority: {task.priority}{task.dueUnix ? ' · Due: '+new Date(task.dueUnix*1000).toLocaleString():''}{task.repeatSeconds ? ' · Repeats every '+Math.round(task.repeatSeconds/60)+' min':''}</small>}
            {['INTERRUPTED','FAILED','NEEDS_REVIEW'].includes(task.state) &&
              <p className="backgroundTaskWarning">Requires review before retrying.</p>}
            {task.needsReview && <button className="subtleButton" type="button"
              onClick={()=>void reapproveScheduledTask(task.taskId)}>Reapprove with a selected file…</button>}
            {task.canPause && <button className="subtleButton" type="button"
              onClick={()=>void changeQueuedBackgroundState(task.taskId,'pause')}>Pause queued</button>}
            {task.canResume && <button className="subtleButton" type="button"
              onClick={()=>void changeQueuedBackgroundState(task.taskId,'resume')}>Resume queued</button>}
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
          Queued tasks can be paused, resumed or cancelled. Running tasks can be cancelled
          with native confirmation, but not paused unless their worker supports it.
          Recovery never automatically replays interrupted work; reapprove using a native file picker.
          Ordinary progress does not invoke the AI model.
        </footer>
      </aside>
    </div>}
    {settingsOpen && <div className="settingsBackdrop" role="presentation" onClick={() => setSettingsOpen(false)}>
      <div className="settingsPane" role="dialog" aria-label="Lu settings" aria-modal="true" onClick={e => e.stopPropagation()}>
        <header><div><h2>Settings</h2><small>Local model management</small></div><button className="quietIcon" onClick={() => setSettingsOpen(false)} aria-label="Close settings">×</button></header>
        <button type="button" className="subtleButton full"
          aria-expanded={advancedSettingsOpen} onClick={() => setAdvancedSettingsOpen(v => !v)}>
          {advancedSettingsOpen ? 'Hide advanced settings' : 'Advanced settings'}
        </button>
        {advancedSettingsOpen && <section className="advancedDocumentSettings"
          aria-label="Default document folder settings">
          <h3>Documents and saved files</h3>
          <p>New text documents save to your user Documents folder by default.
            You may choose a different default folder with the native folder picker.</p>
          <small>System Documents: {documentSettings?.systemDocumentsFolder || 'Unavailable'}</small>
          <code className="writeTarget">{documentSettings?.defaultFolder || 'No default directory resolved'}</code>
          {!documentSettings?.isAvailable &&
            <small role="alert">The default folder is missing or unavailable. Select a valid folder before saving.</small>}
          <div className="settingsActions">
            <button type="button" className="subtleButton" onClick={chooseDefaultDocumentsFolder}>
              Choose default folder
            </button>
            <button type="button" className="subtleButton" onClick={resetDefaultDocumentsFolder}
              disabled={!documentSettings?.isCustom}>
              Reset to system Documents
            </button>
          </div>
          <p className="settingsNotice">Lu saves only after reviewing the exact file contents and destination.
            Modifying existing files requires a separate validated edit/backup pipeline.</p>
        </section>}
        <section className="advancedDocumentSettings" aria-label="Browser visibility settings">
          <h3>Browser visibility</h3>
          <p>Automatic uses a headed Chromium window hidden by default until the user is needed.
             Always Visible is optional. This preference does not launch or authorize browsing.</p>
          <label htmlFor="browserVisibilityMode">Window visibility</label>
          <select id="browserVisibilityMode" value={browserVisibility}
            onChange={e=>{
              const mode=e.target.value;
              invoke('lu_set_browser_visibility',{mode})
                .then(saved=>{setBrowserVisibility(saved);setBrowserPreferenceError('');})
                .catch(error=>setBrowserPreferenceError(String(error)));
            }}>
            <option value="automatic_hidden">Automatic (headed, initially hidden)</option>
            <option value="always_visible">Always visible</option>
          </select>
          {browserPreferenceError&&<small role="alert">{browserPreferenceError}</small>}
          <small>{browserRuntime?.reason||'Checking native browser runtime admission…'}</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={browserBusy} onClick={()=>browserOperation('lu_browser_runtime_status')}>
              Check Browser runtime
            </button>
          </div>
          <label htmlFor="browserTrustedOrigins">HTTPS origins for a new read-only task</label>
          <input id="browserTrustedOrigins" value={browserOrigins}
            onChange={e=>setBrowserOrigins(e.target.value)} disabled={browserBusy||browserRuntime?.running}
            placeholder="https://example.org" />
          <div className="settingsActions">
            {!browserRuntime?.running &&
              <button type="button" disabled={browserBusy||!chatId||!browserRuntime?.launchReady}
                onClick={()=>browserOperation('lu_browser_start',{chatId,origins:browserOrigins})}>
                Authorize read-only Browser
              </button>}
            {browserRuntime?.running && <>
              <button type="button" disabled={browserBusy||browserReadBusy||!runtime.sessionId
                  ||browserRuntime.humanVerificationRequired}
                onClick={()=>void inspectWithBrowserModel()}>
                {browserReadBusy?'Running supervised Browser model read…':
                  'Let Lu inspect this approved browser'}
              </button>

              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_set_visible',{visible:true})}>
                Show browser
              </button>
              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_set_visible',{visible:false})}>
                Hide browser
              </button>
              {browserRuntime.humanVerificationRequired &&
                <button type="button" disabled={browserBusy}
                  onClick={()=>browserOperation('lu_browser_human_verified')}>
                  I've completed verification
                </button>}
              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_stop')}>Stop browser</button>
            </>}
          </div>
          <small>Browser navigation requires a currently authenticated human conversation and separate native approval.
            Lu's supervised read-only inspection uses the same approved turn and exact origins. It grants no form, click, typing or additional network authority.</small>
          {browserReadError&&<small role="alert">{browserReadError}</small>}
          {browserReadResult?.chatId===chatId&&<section aria-label="Model-guided browser observations">
            <strong>{browserReadResult.proposalsExecuted} supervised Browser operations</strong>
            <small>{browserReadResult.message}</small>
            <ul>{browserReadResult.observations.map((page,index)=><li key={index}>
              <strong>{page.status}</strong>
              {page.url&&<small>{page.url}</small>}
              {page.excerpt&&<pre>{page.excerpt}</pre>}
              {page.truncated&&<small>Page excerpt truncated.</small>}
            </li>)}</ul>
          </section>}
        </section>
        <section className="advancedDocumentSettings" aria-label="Research discovery">
          <h3>Research · verified-evidence workflow</h3>
          <p>OpenAlex discovery is a separately approved read-only operation. Search results are
             unverified leads, never citations, until registry and document evidence are checked.</p>
          <div className="settingsActions">
            <button type="button" disabled={!chatId||browserEvidenceBusy}
              onClick={()=>void reviewBrowserEvidence()}>
              {browserEvidenceBusy?'Reading evidence…':'Review observed Browser evidence'}
            </button>
          </div>
          <small>Browser reads are local, untrusted observations, not independently
            corroborated sources or verified academic citations. Their exact
            post-redirect URLs and text digests come from Lu's supervised worker.</small>
          {Array.isArray(browserEvidenceByChat[chatId])&&
            <div className="researchResults" aria-label="Observed Browser evidence">
              <strong>{browserEvidenceByChat[chatId].length} recent Browser observations · UNCORROBORATED</strong>
              <ul>{browserEvidenceByChat[chatId].map(item=><li key={item.callId}>
                <strong>{item.sourceUrl}</strong>
                <small>Observed SHA-256: {item.observedSha256}</small>
                <small>{item.verificationStatus} · citation verified: no</small>
                <pre>{item.excerpt.slice(0,1500)}</pre>
                {item.excerptTruncated&&<small>Excerpt incomplete; only saved text can be checked.</small>}
                <label>Exact passage from this observed excerpt
                  <input value={researchQuoteDraftByKey[chatId+'|'+item.generationId+'|'+item.callId]||''}
                    onChange={e=>{
                      const key=chatId+'|'+item.generationId+'|'+item.callId;
                      setResearchQuoteDraftByKey(old=>({...old,[key]:e.target.value}));
                      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
                    }} placeholder="Paste a verbatim quotation" />
                </label>
                <label>Optional independently corroborated OpenAlex source ID
                  <input value={researchQuoteSourceByKey[chatId+'|'+item.generationId+'|'+item.callId]||''}
                    onChange={e=>{
                      const key=chatId+'|'+item.generationId+'|'+item.callId;
                      setResearchQuoteSourceByKey(old=>({...old,[key]:e.target.value}));
                      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
                    }} placeholder="Only if Crossref metadata was checked" />
                </label>
                <button type="button" disabled={researchQuoteBusy||
                  (researchQuoteDraftByKey[chatId+'|'+item.generationId+'|'+item.callId]||'').trim().length<4}
                  onClick={()=>void verifyObservedQuote(item)}>
                  Check exact source passage
                </button>
                {researchQuoteProofByKey[chatId+'|'+item.generationId+'|'+item.callId]&&
                  <small>{researchQuoteProofByKey[chatId+'|'+item.generationId+'|'+item.callId].message}</small>}
              </li>)}</ul>
            </div>}
          <label htmlFor="researchQuery">Research question or subject</label>
          <input id="researchQuery" value={researchQuery} disabled={researchBusy}
            onChange={e=>{setResearchQuery(e.target.value);setResearchResult(null);}}
            placeholder="e.g., spatial reasoning and GUI automation" />
          <div className="settingsActions">
            <button type="button" disabled={!chatId||!researchAvailable||researchBusy||!researchQuery.trim()}
              onClick={runResearchDiscovery}>
              {researchBusy?'Searching reviewed source…':'Search OpenAlex'}
            </button>
          </div>
          <small>{researchAvailable?'Pinned HTTPS research runtime available. Native consent required.'
            :'Research network runtime is not bundled in this build; discovery remains disabled.'}</small>
          <label>Maximum distinct DOI leads (up to 120)
            <input type="number" min="1" max="120" value={researchCampaignCeiling}
              onChange={e=>setResearchCampaignCeiling(Math.min(120,Math.max(1,
                Number(e.target.value)||1)))} />
          </label>
          <div className="settingsActions">
            <button type="button" disabled={!chatId||!researchAvailable||
              !researchQuery.trim()||researchCampaignBusy}
              onClick={()=>void runResearchCampaign()}>
              {researchCampaignBusy?'Researching in approved catalog…':'Run bounded Research campaign'}
            </button>
            <button type="button" disabled={!chatId||researchCampaignBusy}
              onClick={()=>void refreshResearchCampaigns()}>
              Restore saved research jobs
            </button>
          </div>
          {Array.isArray(researchCampaignsByChat[chatId])&&
            <div className="researchResults" aria-label="Durable research campaigns">
              <ul>{researchCampaignsByChat[chatId].map(job=><li key={job.campaignId}>
                <strong>{job.query}</strong>
                <small>{job.state} · {job.distinctDois}/{job.candidateCeiling}
                  distinct DOI leads · {job.completedPages} pages checkpointed</small>
                <small>{job.message}</small>
                {job.distinctDois>0&&job.state!=='CANCELLED'&&
                  <button type="button" disabled={researchCampaignBusy||!researchAvailable}
                    onClick={()=>void reviewResearchCampaign(job.campaignId)}>
                    Review next 10 discovered sources
                  </button>}
                {researchCampaignReviewByChat[chatId+'|'+job.campaignId]&&
                  <small>Reviewed {researchCampaignReviewByChat[chatId+'|'+job.campaignId].sourcesReviewed}
                    of {researchCampaignReviewByChat[chatId+'|'+job.campaignId].discoveredDois}
                    DOI leads; full document text admitted for
                    {' '}{researchCampaignReviewByChat[chatId+'|'+job.campaignId].completeDocumentTexts}.
                    Unsupported, mismatched and incomplete sources remain uncited.</small>}
                <button type="button" disabled={researchCampaignBusy}
                  onClick={()=>void showResearchReviewRows(job.campaignId)}>
                  Inspect each source outcome
                </button>
                {Array.isArray(researchCampaignRowsByChat[chatId+'|'+job.campaignId])&&
                  <ul>{researchCampaignRowsByChat[chatId+'|'+job.campaignId].map(item=>
                    <li key={item.sourceId}>
                      <small>{item.sourceId}</small>
                      <small>{item.stage} · {item.reasonCode}</small>
                      {item.canRetry&&
                        <button type="button" disabled={researchCampaignBusy}
                          onClick={()=>void retryResearchSource(job.campaignId,item.sourceId)}>
                          Authorize candidate retry
                        </button>}
                    </li>
                  )}</ul>}
                {job.blockedReason&&<small>Blocker: {job.blockedReason}</small>}
                {['PAUSED','BLOCKED','RUNNING'].includes(job.state)&&
                  <button type="button" disabled={researchCampaignBusy||!researchAvailable}
                    onClick={()=>void resumeResearchCampaign(job.campaignId)}>
                    Reauthorize and resume
                  </button>}
                {!['DONE','CANCELLED'].includes(job.state)&&
                  <button type="button" disabled={researchCampaignBusy}
                    onClick={()=>void cancelResearchCampaign(job.campaignId)}>
                    Cancel campaign
                  </button>}
              </li>)}</ul>
            </div>}
          <div className="researchResults" aria-label="Source-linked claim matrix">
            <strong>Document-backed claim and counterevidence review</strong>
            <small>Only a previously verified full-text PDF passage can enter this matrix.
              Your interpretation is recorded as human-reviewed, not as established fact.</small>
            <input aria-label="Verified research source ID"
              placeholder="Crossref-corroborated OpenAlex source ID"
              value={researchClaimDraft.sourceId}
              onChange={e=>setResearchClaimDraft(old=>({...old,sourceId:e.target.value}))}/>
            <input aria-label="Claim under analysis" placeholder="Specific claim"
              value={researchClaimDraft.claim}
              onChange={e=>setResearchClaimDraft(old=>({...old,claim:e.target.value}))}/>
            <textarea aria-label="Verified quote for research claim"
              placeholder="Exact verified PDF passage" value={researchClaimDraft.quote}
              onChange={e=>setResearchClaimDraft(old=>({...old,quote:e.target.value}))}/>
            <label>Evidence relationship
              <select value={researchClaimDraft.stance}
                onChange={e=>setResearchClaimDraft(old=>({...old,stance:e.target.value}))}>
                <option value="SUPPORTS">Supports</option>
                <option value="CHALLENGES">Challenges</option>
                <option value="COMPLICATES">Qualifies or complicates</option>
              </select>
            </label>
            <input aria-label="Reasoning for interpretation"
              placeholder="Why this quote relates to the claim"
              value={researchClaimDraft.reasoning}
              onChange={e=>setResearchClaimDraft(old=>({...old,reasoning:e.target.value}))}/>
            <div className="settingsActions">
              <button type="button" disabled={!chatId||researchReportBusy||
                !researchClaimDraft.sourceId.trim()||!researchClaimDraft.claim.trim()||
                !researchClaimDraft.quote.trim()||!researchClaimDraft.reasoning.trim()}
                onClick={()=>void submitResearchClaim()}>
                Save my evidence interpretation
              </button>
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void refreshResearchClaims()}>
                Review saved claims
              </button>
            </div>
            {Array.isArray(researchClaimsByChat[chatId])&&
              <ul>{researchClaimsByChat[chatId].map(item=><li key={item.claim}>
                <strong>{item.claim}</strong>
                <small>Support: {item.supports} · Challenge: {item.challenges}
                  · Qualification: {item.complicates}
                  · Source IDs: {item.distinctCatalogSources}</small>
                {item.unresolvedDisagreement&&
                  <small>Unresolved: contrary or qualifying evidence remains</small>}
                <small>{item.reasonCodes.join(', ')}</small>
              </li>)}</ul>}
            <label>Verified evidence report title
              <input value={researchReportTitle}
                onChange={e=>setResearchReportTitle(e.target.value)}/>
            </label>
            <label>
              <input type="checkbox" checked={researchReportIncludeBibliography}
                onChange={e=>setResearchReportIncludeBibliography(e.target.checked)}/>
              Include reviewed P07 bibliography ({researchCitationStyle})
            </label>
            <small>The optional bibliography uses only sealed local Crossref receipts;
              it never independently validates publication editions, direct
              quotations, exact pages or factual conclusions.</small>
            <div className="settingsActions">
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void exportResearchReport('markdown')}>
                Save Markdown report
              </button>
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void exportResearchReport('json')}>
                Save evidence JSON
              </button>
            </div>
            {researchReportSavedByChat[chatId]&&
              <small>Saved {researchReportSavedByChat[chatId].format}:
                {' '}{researchReportSavedByChat[chatId].savedPath}
                {' '}· SHA-256: {researchReportSavedByChat[chatId].savedSha256}
                {' '}· {researchReportSavedByChat[chatId].checkedPassages}
                verified passages · {researchReportSavedByChat[chatId].bibliographySources||0}
                reviewed bibliography sources. Interpretations are not independently certified.</small>}
          </div>
          <label>Review citation style
            <select aria-label="Research citation style" value={researchCitationStyle}
              disabled={researchCitationBusy}
              onChange={e=>{
                setResearchCitationStyle(e.target.value);
                setResearchCitationsByChat({});
                setResearchBibliographiesByChat({});
                setResearchBibliographyExportsByChat({});
              }}>
              <option value="apa7">APA 7</option>
              <option value="mla9">MLA 9</option>
              <option value="chicago18">Chicago 18 (notes/bibliography)</option>
              <option value="turabian9">Turabian 9 (notes/bibliography)</option>
              <option value="sbl2">SBL Handbook 2</option>
            </select>
          </label>
          <small>Academic citations require separately corroborated DOI and author identity.
            Review Crossref publication metadata before using it; Lu never invents a
            journal abbreviation, page reference, quotation, or ancient text edition.</small>
          <div className="settingsActions">
            <button type="button" disabled={!chatId||researchBusy||researchBibliographyBusy}
              onClick={()=>void previewResearchBibliography()}>
              {researchBibliographyBusy?'Reviewing bibliography…':'Compile reviewed bibliography offline'}
            </button>
            <button type="button" disabled={!chatId||researchBibliographyBusy}
              onClick={()=>void exportResearchBibliography('markdown')}>
              Save new bibliography Markdown
            </button>
            <button type="button" disabled={!chatId||researchBibliographyBusy}
              onClick={()=>void exportResearchBibliography('csl-json')}>
              Save new bibliography CSL-JSON
            </button>
          </div>
          {researchBibliographiesByChat[chatId]&&
            <div className="researchResults" aria-label="Reviewed citation bibliography preview">
              <strong>{researchBibliographiesByChat[chatId].style} · {
                researchBibliographiesByChat[chatId].sourceCount} pinned sources · REVIEW REQUIRED</strong>
              <small>{researchBibliographiesByChat[chatId].message}</small>
              <pre>{researchBibliographiesByChat[chatId].markdown.slice(0,12000)}</pre>
              {researchBibliographiesByChat[chatId].markdown.length>12000&&
                <small>Preview limited to 12,000 characters. Export includes the full bounded bibliography.</small>}
              <small>Catalog evidence manifest SHA-256: {
                researchBibliographiesByChat[chatId].catalogManifestSha256}</small>
            </div>}
          {researchBibliographyExportsByChat[chatId]&&
            <small role="status">New bibliography saved: {
              researchBibliographyExportsByChat[chatId].savedPath} · SHA-256: {
              researchBibliographyExportsByChat[chatId].savedSha256}. Publication details remain unverified.</small>}
          <div className="researchResults" aria-label="SBL2 Scripture locator only">
            <h4>SBL2 canonical Scripture locator · unverified passage</h4>
            <small>Choose a canonical book and an explicitly named translation.
              This formats a reference only. Lu does not fetch or verify Bible text,
              ancient manuscripts, critical editions or the existence of the verse.
              Modern Bible translations normally do not create a bibliography entry.</small>
            <label>Bible book
              <select aria-label="SBL Scripture book" value={sblScriptureBook}
                onChange={e=>setSblScriptureBook(e.target.value)}>
                <option key="Genesis" value="Genesis">Genesis</option>
                <option key="Exodus" value="Exodus">Exodus</option>
                <option key="Leviticus" value="Leviticus">Leviticus</option>
                <option key="Numbers" value="Numbers">Numbers</option>
                <option key="Deuteronomy" value="Deuteronomy">Deuteronomy</option>
                <option key="Joshua" value="Joshua">Joshua</option>
                <option key="Judges" value="Judges">Judges</option>
                <option key="Ruth" value="Ruth">Ruth</option>
                <option key="1 Samuel" value="1 Samuel">1 Samuel</option>
                <option key="2 Samuel" value="2 Samuel">2 Samuel</option>
                <option key="1 Kings" value="1 Kings">1 Kings</option>
                <option key="2 Kings" value="2 Kings">2 Kings</option>
                <option key="1 Chronicles" value="1 Chronicles">1 Chronicles</option>
                <option key="2 Chronicles" value="2 Chronicles">2 Chronicles</option>
                <option key="Ezra" value="Ezra">Ezra</option>
                <option key="Nehemiah" value="Nehemiah">Nehemiah</option>
                <option key="Esther" value="Esther">Esther</option>
                <option key="Job" value="Job">Job</option>
                <option key="Psalms" value="Psalms">Psalms</option>
                <option key="Proverbs" value="Proverbs">Proverbs</option>
                <option key="Ecclesiastes" value="Ecclesiastes">Ecclesiastes</option>
                <option key="Song of Songs" value="Song of Songs">Song of Songs</option>
                <option key="Isaiah" value="Isaiah">Isaiah</option>
                <option key="Jeremiah" value="Jeremiah">Jeremiah</option>
                <option key="Lamentations" value="Lamentations">Lamentations</option>
                <option key="Ezekiel" value="Ezekiel">Ezekiel</option>
                <option key="Daniel" value="Daniel">Daniel</option>
                <option key="Hosea" value="Hosea">Hosea</option>
                <option key="Joel" value="Joel">Joel</option>
                <option key="Amos" value="Amos">Amos</option>
                <option key="Obadiah" value="Obadiah">Obadiah</option>
                <option key="Jonah" value="Jonah">Jonah</option>
                <option key="Micah" value="Micah">Micah</option>
                <option key="Nahum" value="Nahum">Nahum</option>
                <option key="Habakkuk" value="Habakkuk">Habakkuk</option>
                <option key="Zephaniah" value="Zephaniah">Zephaniah</option>
                <option key="Haggai" value="Haggai">Haggai</option>
                <option key="Zechariah" value="Zechariah">Zechariah</option>
                <option key="Malachi" value="Malachi">Malachi</option>
                <option key="Matthew" value="Matthew">Matthew</option>
                <option key="Mark" value="Mark">Mark</option>
                <option key="Luke" value="Luke">Luke</option>
                <option key="John" value="John">John</option>
                <option key="Acts" value="Acts">Acts</option>
                <option key="Romans" value="Romans">Romans</option>
                <option key="1 Corinthians" value="1 Corinthians">1 Corinthians</option>
                <option key="2 Corinthians" value="2 Corinthians">2 Corinthians</option>
                <option key="Galatians" value="Galatians">Galatians</option>
                <option key="Ephesians" value="Ephesians">Ephesians</option>
                <option key="Philippians" value="Philippians">Philippians</option>
                <option key="Colossians" value="Colossians">Colossians</option>
                <option key="1 Thessalonians" value="1 Thessalonians">1 Thessalonians</option>
                <option key="2 Thessalonians" value="2 Thessalonians">2 Thessalonians</option>
                <option key="1 Timothy" value="1 Timothy">1 Timothy</option>
                <option key="2 Timothy" value="2 Timothy">2 Timothy</option>
                <option key="Titus" value="Titus">Titus</option>
                <option key="Philemon" value="Philemon">Philemon</option>
                <option key="Hebrews" value="Hebrews">Hebrews</option>
                <option key="James" value="James">James</option>
                <option key="1 Peter" value="1 Peter">1 Peter</option>
                <option key="2 Peter" value="2 Peter">2 Peter</option>
                <option key="1 John" value="1 John">1 John</option>
                <option key="2 John" value="2 John">2 John</option>
                <option key="3 John" value="3 John">3 John</option>
                <option key="Jude" value="Jude">Jude</option>
                <option key="Revelation" value="Revelation">Revelation</option>
              </select>
            </label>
            <label>Translation or version
              <select aria-label="SBL Scripture translation" value={sblScriptureTranslation}
                onChange={e=>setSblScriptureTranslation(e.target.value)}>
                {['NRSV','RSV','NIV','NASB','KJV','CEB'].map(v=><option key={v}>{v}</option>)}
              </select>
            </label>
            <div className="settingsActions">
              <label>Chapter
                <input type="number" min="1" max="150" value={sblScriptureChapter}
                  onChange={e=>setSblScriptureChapter(e.target.value)}/>
              </label>
              <label>First verse
                <input type="number" min="1" max="176" value={sblScriptureFirstVerse}
                  onChange={e=>setSblScriptureFirstVerse(e.target.value)}/>
              </label>
              <label>Last verse (optional)
                <input type="number" min="1" max="176" value={sblScriptureLastVerse}
                  onChange={e=>setSblScriptureLastVerse(e.target.value)}/>
              </label>
            </div>
            <button type="button" disabled={!chatId}
              onClick={()=>void previewSblScriptureLocator()}>
              Format SBL2 Scripture reference locally
            </button>
            {sblScripturePreview&&
              <div>
                <strong>{sblScripturePreview.reference}</strong>
                <small>Passage text not checked; translation and edition unverified.
                  This is not a bibliography item or scholarly source attestation.</small>
                <button type="button"
                  onClick={()=>void navigator.clipboard.writeText(sblScripturePreview.reference)
                    .catch(e=>setResearchError(String(e)))}>
                  Copy Scripture reference
                </button>
              </div>}
            <label>Critical-edition reference (not a manuscript-witness check)
              <select aria-label="SBL critical edition" value={sblEditionId}
                onChange={e=>{setSblEditionId(e.target.value);setSblEditionPreview(null);}}>
                <option value="NA28">Novum Testamentum Graece, 28th rev. edition (Greek NT)</option>
                <option value="RAHLFS-HANHART-2006">Rahlfs–Hanhart, Septuaginta, 2006 (Greek OT)</option>
              </select>
            </label>
            <button type="button" disabled={!chatId}
              onClick={()=>void previewSblCriticalEdition()}>
              Format selected critical-edition reference
            </button>
            {sblEditionPreview&&
              <div className="researchResults" aria-label="Critical edition identification only">
                <strong>{sblEditionPreview.reference}</strong>
                <small>{sblEditionPreview.editionTitle} · {sblEditionPreview.publisher} · {
                  sblEditionPreview.publicationYear} · ISBN {sblEditionPreview.isbn}</small>
                <small>Edition catalog: {sblEditionPreview.catalogUrl}</small>
                <small>Text readings, verse-numbering equivalences, variant witnesses,
                  translations and any specific manuscript remain UNVERIFIED.</small>
                <button type="button" onClick={()=>void navigator.clipboard.writeText(
                  sblEditionPreview.reference
                ).catch(e=>setResearchError(String(e)))}>
                  Copy critical-edition reference
                </button>
              </div>}
          </div>
          {researchError&&<small role="alert">{researchError}</small>}
          {researchResult?.chatId===chatId&&<div className="researchResults" aria-label="Research evidence leads">
            <strong>{researchResult.sourceCount} discovered sources · UNVERIFIED</strong>
            <small>{researchResult.message}</small>
            <ul>{researchResult.leads.slice(0,25).map(l=><li key={l.sourceId}>
              <strong>{l.title}</strong>
              <small>Source ID: {l.sourceId}</small>
              <small>{l.authors} · DOI {l.doi} · {
                researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus || l.verificationStatus
              }</small>
              <button type="button" disabled={researchBusy||!researchAvailable}
                onClick={()=>void checkResearchMetadata(l.sourceId)}>
                Check independent Crossref metadata
              </button>
              {researchMetadataByChat[chatId+'|'+l.sourceId]&&
                <small>{researchMetadataByChat[chatId+'|'+l.sourceId].message}</small>}
              {researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus==='metadata_corroborated'&&
                <button type="button" disabled={researchBusy||!researchAvailable}
                  onClick={()=>void retrieveResearchDocument(l.sourceId)}>
                  Retrieve bounded PDF/HTML document
                </button>}
              {researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus==='metadata_corroborated'&&
                <button type="button" disabled={researchBusy||researchCitationBusy||!researchAvailable}
                  onClick={()=>void previewResearchCitation(l.sourceId)}>
                  {researchCitationBusy?'Reviewing catalog…':'Preview evidence-bound citation'}
                </button>}
              {researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus==='metadata_corroborated'&&
                <button type="button" disabled={researchBusy||researchCitationBusy||!researchAvailable}
                  onClick={()=>void reviewResearchCitationRevision(l.sourceId)}>
                  Review changed Crossref metadata (prior version remains until approval)
                </button>}
              {researchCitationRevisionByChat[chatId+'|'+l.sourceId]&&
                <small role="status">Citation catalog revision #{
                  researchCitationRevisionByChat[chatId+'|'+l.sourceId].revisionCount} approved.
                  Previous SHA-256: {researchCitationRevisionByChat[chatId+'|'+l.sourceId].previousSha256};
                  current SHA-256: {researchCitationRevisionByChat[chatId+'|'+l.sourceId].currentSha256}.
                  Publication details and passages still need independent review.</small>}
              {researchCitationsByChat[chatId+'|'+l.sourceId]&&
                <div className="researchResults" aria-label="Crossref-backed citation preview">
                  <strong>{researchCitationsByChat[chatId+'|'+l.sourceId].style} · REVIEW REQUIRED</strong>
                  <pre>{researchCitationsByChat[chatId+'|'+l.sourceId].bibliography}</pre>
                  {researchCitationsByChat[chatId+'|'+l.sourceId].firstNote&&
                    <small>First note: {researchCitationsByChat[chatId+'|'+l.sourceId].firstNote}</small>}
                  <small>Crossref metadata response SHA-256:
                    {' '}{researchCitationsByChat[chatId+'|'+l.sourceId].catalogResponseSha256}</small>
                  <small>{researchCitationsByChat[chatId+'|'+l.sourceId].message}</small>
                  <details>
                    <summary>CSL-JSON-compatible source record · evidence limitations included</summary>
                    <pre>{JSON.stringify(researchCitationsByChat[chatId+'|'+l.sourceId].cslJson,null,2)}</pre>
                    <button type="button"
                      onClick={()=>void navigator.clipboard.writeText(
                        JSON.stringify(researchCitationsByChat[chatId+'|'+l.sourceId].cslJson,null,2)
                      ).catch(e=>setResearchError(String(e)))}>
                      Copy reviewed CSL JSON
                    </button>
                  </details>
                  <small>Bibliographic identity matched; publication particulars, quotations,
                    pages, and interpretations remain unverified. This is a preview,
                    not a final scholarly citation.</small>
                </div>}
              {researchDocumentActive?.chatId===chatId
                &&researchDocumentActive?.sourceId===l.sourceId&&
                <button type="button" disabled={!researchBusy}
                  onClick={()=>void cancelResearchDocument()}>
                  Cancel active HTTPS document read
                </button>}
              {researchDocumentsByChat[chatId+'|'+l.sourceId]&&
                <div className="researchResults" aria-label="Retrieved research document">
                  <small>{researchDocumentsByChat[chatId+'|'+l.sourceId].message}</small>
                  <small>Final HTTPS URL: {researchDocumentsByChat[chatId+'|'+l.sourceId].finalUrl}</small>
                  <small>Redirect-chain SHA-256: {researchDocumentsByChat[chatId+'|'+l.sourceId].redirectChainSha256}</small>
                  <small>{researchDocumentsByChat[chatId+'|'+l.sourceId].format} · {researchDocumentsByChat[chatId+'|'+l.sourceId].documentAccessed?'Document text accessed':'Unverified document candidate'}</small>
                  <pre>{researchDocumentsByChat[chatId+'|'+l.sourceId].excerpt}</pre>
                  {researchDocumentsByChat[chatId+'|'+l.sourceId].documentAccessed&&
                    <>
                      <input aria-label="Exact quote from retrieved document"
                        value={researchDocumentQuotes[chatId+'|'+l.sourceId]||''}
                        onChange={e=>{
                          const key=chatId+'|'+l.sourceId;
                          setResearchDocumentQuotes(old=>({...old,[key]:e.target.value}));
                          setResearchDocumentPassages(old=>({...old,[key]:null}));
                        }} placeholder="Copy an exact PDF passage" />
                      <button type="button" disabled={researchBusy||
                        !(researchDocumentQuotes[chatId+'|'+l.sourceId]||'').trim()}
                        onClick={()=>void verifyResearchDocumentPassage(l.sourceId)}>
                        Verify stored PDF passage
                      </button>
                      {researchDocumentPassages[chatId+'|'+l.sourceId]&&
                        <small>Passage verified against stored PDF text; model interpretation remains unverified.</small>}
                    </>}
                </div>}
            </li>)}</ul>
            <div className="settingsActions">
              <button type="button" disabled={researchBusy||researchResult.page<=1}
                onClick={()=>void runResearchDiscovery(researchResult.page-1)}>
                Previous discovery page
              </button>
              <small>Page {researchResult.page} of at most 5 · leads are not citations</small>
              <button type="button" disabled={researchBusy||!researchResult.nextPage}
                onClick={()=>void runResearchDiscovery(researchResult.nextPage)}>
                Next discovery page
              </button>
            </div>
          </div>}
        </section>
        <section className="researchResults" aria-label="Non-destructive Office text replacement">
          <h3>Word and PowerPoint text editing</h3>
          <small>Replace matching complete text runs across a DOCX or PPTX,
            preserving the original document and non-target package content.
            Text spanning separately styled runs is not guessed or merged.</small>
          <label>Document format
            <select value={wordReplaceFormat}
              onChange={e=>{setWordReplaceFormat(e.target.value);setWordReplaceReceipt(null);}}>
              <option value="docx">Word (.docx)</option>
              <option value="pptx">PowerPoint (.pptx)</option>
            </select>
          </label>
          <label>Exact existing text
            <input value={wordReplaceOld}
              onChange={e=>setWordReplaceOld(e.target.value)}/>
          </label>
          <label>Replacement text
            <input value={wordReplaceNew}
              onChange={e=>setWordReplaceNew(e.target.value)}/>
          </label>
          <button type="button" disabled={!chatId||wordReplaceBusy||
            !wordReplaceOld.trim()||!wordReplaceNew.trim()||
            wordReplaceOld===wordReplaceNew}
            onClick={()=>void replaceWordTextRuns()}>
            {wordReplaceBusy?'Preparing exact Office revision…':
              'Select '+(wordReplaceFormat==='docx'?'Word':'PowerPoint')+' document and replace'}
          </button>
          {wordReplaceReceipt&&<small role="status">
            Created {wordReplaceReceipt.newFileName}: {wordReplaceReceipt.replacements}
            replacements · SHA-256 {wordReplaceReceipt.revisedSha256}.
            Original file was not overwritten.
          </small>}
        </section>
        <h3>Models</h3><p>GGUF models are discovered only from Lu's existing model store, never loaded automatically. Searching below is local and read-only.</p>
        <label className="modelSearchLabel">Search registered local models
          <input type="search" value={modelSearch} maxLength={100} autoComplete="off"
            placeholder="Filename…" onChange={event=>setModelSearch(event.target.value)}/>
        </label>
        <button className="subtleButton full" onClick={rescanModels} disabled={busy || loaded}>Rescan models folder</button>
        <button className="subtleButton full" onClick={importModel} disabled={busy}>Import GGUF from another location</button>
        <div className="settingsModelList">{models.filter(m=>m.filename.toLocaleLowerCase().includes(modelSearch.trim().toLocaleLowerCase()))
          .map(m => <button key={m.modelRef} className={'settingsModel '+(selectedModel === m.modelRef ? 'selected' : '')}
          onClick={() => setSelectedModel(m.modelRef)} disabled={busy}>
          <strong>{m.filename}</strong><small>{m.projectorAttached ? 'GGUF + MMProj' : 'GGUF only'} · {m.available ? 'Available' : 'Missing file'}</small></button>)}</div>
        {modelInspection?.modelRef===selectedModel&&<section className="modelInspection" aria-label="Local model compatibility inspection">
          <strong>Pre-load compatibility: {modelInspection.status.replaceAll('_',' ')}</strong>
          <small>{modelInspection.detail}</small>
          {modelInspection.fileBytes!=null&&<small>
            GGUF v{modelInspection.ggufVersion} · {(modelInspection.fileBytes/1048576).toFixed(1)} MiB on disk ·
            {modelInspection.tensorCount} header tensors ·
            MMProj {modelInspection.projectorAttached?(modelInspection.projectorChecked?'header checked':'unavailable'):'not paired'}
          </small>}
          {modelInspection.architecture&&<small>GGUF-reported architecture: {modelInspection.architecture}
            {modelInspection.tokenizerModel?' · tokenizer: '+modelInspection.tokenizerModel:''}
            {modelInspection.reportedContextLength?' · reported context: '+modelInspection.reportedContextLength.toLocaleString()+' tokens':''}
            {modelInspection.ggufFileType!=null?' · GGUF file-type code: '+modelInspection.ggufFileType:''}
          </small>}
          <small>Backend: {modelInspection.backendRequested}. Metadata is untrusted and does not certify architecture support, license or GPU/VRAM compatibility.</small>
        </section>}
        <button className="subtleButton full" disabled={busy || loaded || !selected || selected.projectorAttached} onClick={attachProjector}>Attach MMProj to selected model</button>
        <div className="settingsActions"><button className="primaryButton" disabled={busy || loaded || !selected?.available || !modelInspection?.loadAllowed || modelInspection.modelRef!==selectedModel} onClick={loadModel}>Load inspected model</button>
          <button className="subtleButton" disabled={busy || !loaded} onClick={unloadModel}>Unload current model</button></div>
        <div className="settingsStatus"><span className="statusDot" data-ready={loaded}></span>{activity || (loaded ? 'Ready' : 'No model loaded')}</div>
        <section aria-label="Public GGUF model discovery" className="modelProviderReview">
          <h4>Public model discovery (Hugging Face)</h4>
          <small>Explicit search uses Lu's separately hash-pinned public HTTPS reader. It cannot send credentials or download weights.</small>
          <label>Search public model repositories
            <input value={providerQuery} maxLength={96} autoComplete="off"
              placeholder="e.g. qwen gguf"
              onChange={event=>setProviderQuery(event.target.value)}/>
          </label>
          <button type="button" className="subtleButton" disabled={providerBusy||!providerQuery.trim()}
            onClick={()=>void searchPublicModels()}>
            {providerBusy?'Inspecting public catalog…':'Search public GGUF repositories'}
          </button>
          {providerFailure&&<small role="alert">{providerFailure}</small>}
          {!!providerHits.length&&<div aria-label="Provider model search results">
            {providerHits.map(lead=><button key={lead.repoId+lead.revision}
              className="settingsModel" type="button" disabled={providerBusy}
              onClick={()=>void inspectPublicModel(lead)}>
              <strong>{lead.repoId}</strong>
              <small>Revision {lead.revision.slice(0,12)} · reported license {lead.licenseLabel} · inspect files</small>
            </button>)}
          </div>}
          {providerReview&&<div className="modelInspection" aria-label="Pinned model file review">
            <strong>{providerReview.repoId}</strong>
            <small>Exact revision {providerReview.revision} · provider-reported license {providerReview.licenseLabel}</small>
            {providerReview.files.map(file=><div key={file.name}>
              <strong>{file.name}</strong>
              <small>{(file.bytes/1048576).toFixed(1)} MiB · SHA-256 {file.sha256}</small>
              <button type="button" className="subtleButton" disabled={providerBusy}
                onClick={()=>void preparePinnedTransfer(file)}>
                Review immutable future download plan (does not download)
              </button>
            </div>)}
            {providerTransferIntent&&<small role="status">
              Metadata reviewed for {providerTransferIntent.filename} · expected SHA-256 {providerTransferIntent.sha256}.
              Transfer state: awaiting P05; no file has been downloaded or installed.
            </small>}
            <small>Read-only preview. The license and model architecture require your review. In-app downloads are blocked until the P05 native Transfer Manager can validate HTTPS redirects, bytes, cancellation, resumable identity and separate approval.</small>
            <a href={providerReview.sourceUrl} rel="noopener noreferrer" target="_blank">View pinned repository externally</a>
          </div>}
        </section>
        <section aria-label="Model backend availability">
          <h4>Inference backends</h4>
          {modelBackends.map(item=><div className="modelInspection" key={item.backend}>
            <strong>{item.engine} — {item.backend}</strong>
            <small>{item.selectable?'Current pinned engine path':'Unavailable; not selectable'} ·
              {item.gpuExecutionVerified?' GPU proven':' GPU not proven'} · {item.detail}</small>
          </div>)}
          <small>Only the currently packaged pinned engine is available. Lu will not silently switch to ROCm, CUDA, vLLM or CPU on a strict backend request.</small>
        </section>
        <p className="settingsNotice">A remote search result never installs, imports or loads a model. Use the existing native GGUF import picker for files you have downloaded and checked. The pinned engine requests Vulkan on Windows; driver/offload verification remains a separate physical gate.</p>
      </div>
    </div>}
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
