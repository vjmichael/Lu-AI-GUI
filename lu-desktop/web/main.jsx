import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { splitModelOutput } from './response_view.mjs';
// Host classifies original user intent; no frontend filesystem routing.
import {explainToolOutcome,explainReviewedContinuation} from './reviewed_tool_feedback.mjs';
import {explainMemoryProjectionRecovery} from './reviewed_memory_recovery.mjs';
import { fullEditChangeLines } from './edit_diff.mjs';
import { MarkdownView } from './markdown_view.jsx';
import {buildSubmittedPrompt, attachmentSummary, MAX_TEXT_FILES} from './attachment_drafts.mjs';
import {isPotentialImage, normalizedImageFile} from './image_attachment_input.mjs';
import {stagedImageDimensions,REGION_IMAGE_OPERATIONS,
  imagePointerPixel,imageDragRectangle} from './image_assembly_geometry.mjs';
import {queuedJob, startedJob, tokenForJob, removeJob} from './generation_jobs.mjs';
import {nearTranscriptBottom, restoredScrollTop, isSendKey, isSearchKey, isNewChatKey, matchesChatTitle} from './chat_ui_behavior.mjs';
import {MAX_HISTORY_ROWS,mergeHistoryWindow,captureHistoryAnchor,restoreHistoryAnchor} from './history_window_geometry.mjs';
import {MESSAGE_REACTIONS,DEFAULT_APPEARANCE,CHAT_APPEARANCE_PRESETS,safeAppearance,textForBackground,visibleReactionBySeq} from './message_interactions.mjs';
import {revisionsBySequence,canEdit,provenanceLabel,draftReplyTarget,correctedMessageFollowUp} from './message_revisions.mjs';
import {parseHumanReplanOutline,humanReplanDefaultOutline,REPLAN_REASONS} from './agent_replan.mjs';
import {SettingsWindow,ModelLibraryWindow} from './native_windows.jsx';

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
  const [requiredName,setRequiredName]=useState('');
  const [requiredVersion,setRequiredVersion]=useState('');
  const [requiredPlan,setRequiredPlan]=useState([]);
  const [beliefSubject,setBeliefSubject]=useState('model');
  const [beliefPredicate,setBeliefPredicate]=useState('backend');
  const [beliefVersion,setBeliefVersion]=useState('1.0');
  const [beliefValue,setBeliefValue]=useState('');
  const [beliefSource,setBeliefSource]=useState('');
  const [beliefCorrection,setBeliefCorrection]=useState('');
  const [beliefCorrectionReason,setBeliefCorrectionReason]=useState('');
  const [beliefRows,setBeliefRows]=useState([]);
  const [beliefBusy,setBeliefBusy]=useState(false);

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
      setRequiredPlan([]);
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
  async function planRequiredAsset(e){
    e.preventDefault();
    if(!activeChatId||!resumeProject.trim()||!requiredName.trim()||
       !requiredVersion.trim()||resumeBusy)return;
    setResumeBusy(true);setError('');
    try{
      const result=await invoke('lu_m3_plan_required_assets',{
        chatId:activeChatId,projectId:resumeProject.trim(),
        requirements:[{logicalName:requiredName.trim(),
          version:requiredVersion.trim()}]
      });
      setRequiredPlan(result);
    }catch(e){setError('Required package verification unavailable: '+String(e))}
    finally{setResumeBusy(false)}
  }
  async function readBeliefs(){
    if(!activeChatId||beliefBusy||!resumeProject.trim()||!resumeTask.trim())return;
    setBeliefBusy(true);setError('');
    try{
      const records=await invoke('lu_m4_read_beliefs',{
        chatId:activeChatId,projectId:resumeProject.trim(),taskId:resumeTask.trim(),
        subject:beliefSubject.trim(),predicate:beliefPredicate.trim(),
        applicationVersion:beliefVersion.trim()
      });
      setBeliefRows(records);
    }catch(e){setError('Original belief review unavailable: '+String(e))}
    finally{setBeliefBusy(false)}
  }
  async function noteBelief(e){
    e.preventDefault();
    const sourceId=beliefSource.trim()||
      (reading?.sourceChatId===activeChatId?reading.sourceId:null);
    if(!activeChatId||!sourceId||!beliefValue.trim()||beliefBusy)return;
    setBeliefBusy(true);setError('');
    try{
      await invoke('lu_m4_note_reported_claim',{
        chatId:activeChatId,sourceId,projectId:resumeProject.trim(),
        taskId:resumeTask.trim(),subject:beliefSubject.trim(),
        predicate:beliefPredicate.trim(),value:beliefValue.trim(),
        applicationVersion:beliefVersion.trim()
      });
      setBeliefValue('');setBeliefRows([]);
    }catch(e){setError('Source-anchored claim not recorded: '+String(e))}
    finally{setBeliefBusy(false)}
  }
  async function retractBelief(e){
    e.preventDefault();
    if(!activeChatId||!beliefCorrection.trim()||!beliefCorrectionReason.trim()||
       beliefBusy)return;
    setBeliefBusy(true);setError('');
    try{
      await invoke('lu_m4_retract_claim',{
        chatId:activeChatId,claimId:beliefCorrection.trim(),
        correctionSourceId:beliefSource.trim()||
          (reading?.sourceChatId===activeChatId?reading.sourceId:''),
        reason:beliefCorrectionReason.trim()
      });
      setBeliefCorrection('');setBeliefCorrectionReason('');
      setBeliefRows([]);
    }catch(e){setError('Human-sourced correction unavailable: '+String(e))}
    finally{setBeliefBusy(false)}
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

    <details className="ansfAssetsMemory" aria-label="M4 belief revision and semantic claims">
      <summary>M4 semantic claims · source and conflict review</summary>
      <small>Reported claims are never verified facts. Contradictions, stale versions and corrections must be reviewed; prior claims give no authority to use tools.</small>
      <form className="ansfAssetForm" onSubmit={noteBelief}>
        <label>Project and task
          <small>{resumeProject} · {resumeTask}</small>
        </label>
        <label>Subject key
          <input value={beliefSubject} maxLength={128}
            onChange={e=>setBeliefSubject(e.target.value)}/>
        </label>
        <label>Predicate key
          <input value={beliefPredicate} maxLength={128}
            onChange={e=>setBeliefPredicate(e.target.value)}/>
        </label>
        <label>Application version
          <input value={beliefVersion} maxLength={128}
            onChange={e=>setBeliefVersion(e.target.value)}/>
        </label>
        <label>Original user ANSF source ID
          <input value={beliefSource||
            (reading?.sourceChatId===activeChatId?reading?.sourceId:'')}
            onChange={e=>setBeliefSource(e.target.value)} maxLength={224}
            placeholder="Select an original user message"/>
        </label>
        <label>Reported fact or hypothesis
          <textarea rows={2} maxLength={1024} value={beliefValue}
            onChange={e=>setBeliefValue(e.target.value)}/>
        </label>
        <button type="submit" disabled={beliefBusy||!beliefValue.trim()||
          !(beliefSource||(reading?.sourceChatId===activeChatId&&reading?.sourceId))}>
          Record as reported, not verified</button>
      </form>
      <button type="button" onClick={()=>void readBeliefs()}
        disabled={beliefBusy||!beliefSubject.trim()||!beliefPredicate.trim()}>
        Recheck current source, versions and contradictions</button>
      {beliefRows.map(x=><article className="ansfHistoryEntry" key={x.claimId}>
        <small>{x.status} · original {x.originalSourceId}</small>
        <p>{x.reportedValue}</p><code>{x.claimId}</code>
      </article>)}
      <form className="ansfAssetForm" onSubmit={retractBelief}>
        <label>Claim ID to retract
          <input maxLength={128} value={beliefCorrection}
            onChange={e=>setBeliefCorrection(e.target.value)}/>
        </label>
        <label>Reason for correction
          <input maxLength={512} value={beliefCorrectionReason}
            onChange={e=>setBeliefCorrectionReason(e.target.value)}/>
        </label>
        <button type="submit" disabled={beliefBusy||!beliefCorrection.trim()||
          !beliefCorrectionReason.trim()}>Retract with selected human original…</button>
      </form>
      <small>To read this in a real local-model turn, send: Recall belief project {resumeProject} task {resumeTask} subject {beliefSubject} predicate {beliefPredicate} version {beliefVersion}. This recalls historical evidence, not current permissions.</small>
    </details>
    <details className="ansfAssetsMemory" aria-label="Project continuity">
      <summary>Continue previous work · project checkpoint</summary>
      <small>Source-linked task progress remains available across Lu conversations and model changes. Historical work does not grant a new tool operation. For automatic read-only recovery in a new human chat, send: Resume project lu task windows-installer (use your exact saved project and task IDs).</small>
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
        <strong>Required toolchain recovery plan</strong>
        <small>List an exact required package and version. Verified existing bytes can be reused after fresh tool authorization; missing, changed and unrecorded assets need a new human-approved acquisition. This check never installs anything.</small>
        <form className="ansfAssetForm" onSubmit={planRequiredAsset}>
          <label>Required package
            <input value={requiredName} maxLength={180}
              onChange={e=>setRequiredName(e.target.value)}
              placeholder="Windows SDK package"/>
          </label>
          <label>Exact required version
            <input value={requiredVersion} maxLength={180}
              onChange={e=>setRequiredVersion(e.target.value)}
              placeholder="Previously approved version"/>
          </label>
          <button type="submit" disabled={resumeBusy||!requiredName.trim()||
            !requiredVersion.trim()}>Verify required dependency</button>
        </form>
        {requiredPlan.map(x=><small key={x.logicalName+'-'+x.version} role="status">
          {x.logicalName} {x.version} · {x.state} · {x.nextAction}
        </small>)}
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
  // UX-CHAT01: per-chat decoration state does not enter model context.
  const [messageDecorationsByChat,setMessageDecorationsByChat]=useState({});
  const [messageReplay,setMessageReplay]=useState(null);
  const [revisionHistoryByChat,setRevisionHistoryByChat]=useState({});
  const [revisionPendingByChat,setRevisionPendingByChat]=useState({});
  const [messageEditor,setMessageEditor]=useState(null);
  const [messageProvenance,setMessageProvenance]=useState(null);
  const [linkedReplyTarget,setLinkedReplyTarget]=useState(null);
  const [linkedReplyText,setLinkedReplyText]=useState('');
  const [revisionActionBusy,setRevisionActionBusy]=useState(false);
  const [messageDecorationBusy,setMessageDecorationBusy]=useState(false);
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
  const [reviewedMemoryRecoveryByChat,setReviewedMemoryRecoveryByChat]=useState({});
  const [agentProgressByChat,setAgentProgressByChat]=useState({});
  const [agentRecoveryByPlan,setAgentRecoveryByPlan]=useState({});
  const [recoveryProvenanceByChat,setRecoveryProvenanceByChat]=useState({});
  const [recoveryProvenanceBusy,setRecoveryProvenanceBusy]=useState(false);
  const [agentReviewBusy,setAgentReviewBusy]=useState(false);
  const [agentStepOpByKey,setAgentStepOpByKey]=useState({});
  const [replanOutlineByPlan,setReplanOutlineByPlan]=useState({});
  const [replanReasonByPlan,setReplanReasonByPlan]=useState({});
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
  // One-turn model-side plan proposal. No tool grants or automatic execution.
  const [planDraftNextSend,setPlanDraftNextSend] = useState(false);
  // One Send only. Native host shows an OS picker and reviews exact program
  // and selected resource; no JS/model path can become execution authority.
  const [reviewedAppNextSend,setReviewedAppNextSend] = useState(null);
  // Explicit one-turn Browser opt-in; native dialog remains the grant.
  const [browserResearchNextSend,setBrowserResearchNextSend]=useState(false);
  const [modelLoadTrace, setModelLoadTrace] = useState(null);
  const [modelMemorySample,setModelMemorySample] = useState(null);
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
  const [modelLibraryOpen,setModelLibraryOpen] = useState(false);
  const [modelLibraryTab,setModelLibraryTab] = useState('installed');
  const [reactionPickerSeq,setReactionPickerSeq] = useState(null);
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
        const [trace,allocation]=await Promise.all([
          invoke('lu_load_diagnostics'),invoke('lu_load_memory_sample')
        ]);
        if (mounted) {
          setModelLoadTrace(trace);
          setModelMemorySample(allocation);
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
    if ((!settingsOpen && !modelLibraryOpen) || !selectedModel) { setModelInspection(null); return; }
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
  },[settingsOpen,modelLibraryOpen,selectedModel,models]);

  async function refreshCatalog() {
    const [knownModels, status] = await Promise.all([invoke('lu_list_models'), invoke('lu_status')]);
    setModels(knownModels);
    setRuntime(status);
    setSelectedModel(old => old && knownModels.some(m => m.modelRef === old) ? old : status.modelRef || knownModels[0]?.modelRef || '');
  }
  // Native Settings can attach a separately OS-approved one-shot WGC frame
  // to the SAME active conversation. This event is not a grant: the host
  // independently validates imageRef ownership, source provenance, local
  // egress and projector admission on the next ordinary Send.
  useEffect(()=>{
    let alive=true,off=null;
    listen('lu-observed-image-admitted',event=>{
      const image=event.payload;
      if(!alive||!image||typeof image.imageRef!=='string'||
         image.imageRef.length>128||
         !['image/png','image/jpeg'].includes(image.mimeType)||
         image.chatId!==currentChat.current)return;
      if(draftImages.current.some(item=>item.imageRef===image.imageRef))return;
      if(draftImages.current.length>=4){
        setError('Maximum four image attachments. Remove one before attaching an observed frame.');
        return;
      }
      const original={imageRef:image.imageRef,mimeType:image.mimeType,
        name:'Approved local observation',size:0};
      const next=[...draftImages.current,original];
      draftImages.current=next;setImages(next);
      const prior=pendingDrafts.current.get(image.chatId)||{prompt:draftText.current,files:draftFiles.current};
      pendingDrafts.current.set(image.chatId,{...prior,images:next});
      setActivity('Native-approved local frame attached. It remains local; the model has not analyzed it yet.');
    }).then(unsubscribe=>{if(alive)off=unsubscribe;else unsubscribe();})
      .catch(()=>{});
    return ()=>{alive=false;off?.();};
  },[]);

  useEffect(()=>{
    let alive=true,off=null;
    listen('lu-model-catalog-changed',()=>{
      if(alive)void refreshCatalog().catch(error=>setError(String(error)));
    }).then(fn=>{if(alive)off=fn;else fn();}).catch(()=>{});
    const focused=()=>{if(alive)void refreshCatalog().catch(()=>{});};
    window.addEventListener('focus',focused);
    return ()=>{alive=false;off?.();window.removeEventListener('focus',focused);};
  },[]);
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
    let stopQueued = () => {}, stopStart = () => {}, stopToken = () => {}, stopReadProposal = () => {}, stopWriteProposal = () => {}, stopEditProposal = () => {}, stopNativeAction = () => {}, stopToolProgress = () => {}, stopTurnCapabilities = () => {}, stopBackground = () => {}, stopFinished = () => {}, stopBrowserChallenge = () => {}, stopPlanAdmission = () => {}, stopAgentWitness = () => {};
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
    listen('lu-agent-plan-admission',e=>{
      const result=e.payload;
      if(!alive||!result?.chatId)return;
      if(currentChat.current!==result.chatId)return;
      if(result.status==='review_only_draft'&&result.planId){
        setActivity('Lu recorded a review-only plan draft. Open Tool activity to review the original plan. No worker has executed.');
      }else if(result.status==='not_admitted'){
        setError('No plan was admitted: '+String(result.reason||'Invalid model plan'));
      }
    }).then(fn=>{if(alive)stopPlanAdmission=fn;else fn();});
    listen('lu-agent-step-verified',e=>{
      const result=e.payload;
      if(!alive||!result?.chatId||!result?.planId)return;
      if(currentChat.current===result.chatId){
        setActivity('Step '+(result.stepIndex+1)+' recorded an independently checked '+
          result.status+' witness ('+result.method+'). Another step needs new authorization.');
        void refreshToolActivity(result.chatId);
      }
    }).then(fn=>{if(alive)stopAgentWitness=fn;else fn();});
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
    return () => { alive = false; stopQueued(); stopStart(); stopToken(); stopReadProposal(); stopWriteProposal(); stopEditProposal(); stopNativeAction(); stopToolProgress(); stopTurnCapabilities(); stopBackground(); stopFinished(); stopBrowserChallenge(); stopPlanAdmission(); stopAgentWitness(); };
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
      // Both reads are host-selected and independent. A partial/malformed
      // agent ledger cannot hide already verified native tool audit records.
      const results=await Promise.allSettled([
        invoke('lu_tool_turn_history',{chatId:targetChat}),
        invoke('lu_agent_plan_progress',{chatId:targetChat}),
        invoke('lu_reviewed_memory_recovery_status',{chatId:targetChat})
      ]);
      if(results[0].status==='fulfilled'){
        setToolHistoryByChat(prev=>({...prev,[targetChat]:results[0].value||[]}));
      }
      if(results[1].status==='fulfilled'){
        setAgentProgressByChat(prev=>({...prev,[targetChat]:results[1].value||[]}));
      }
      if(results[2].status==='fulfilled'){
        setReviewedMemoryRecoveryByChat(prev=>({...prev,[targetChat]:results[2].value||[]}));
      }
      const failures=results.filter(x=>x.status==='rejected');
      if(failures.length && currentChat.current===targetChat){
        setError('Some durable task evidence is unavailable: '+
          failures.map(x=>String(x.reason)).join(' | '));
      }
    }finally{setToolHistoryBusy(false)}
  }
  async function reviewAgentPlan(plan,decision){
    if(!chatId||!plan?.revisionSha256)return;
    setAgentReviewBusy(true);
    try{
      const acted=await invoke('lu_agent_review_plan',{
        chatId,planId:plan.planId,revisionSha:plan.revisionSha256,
        decision
      });
      if(acted)await refreshToolActivity(chatId);
    }catch(error){
      setError('Plan review not recorded: '+String(error));
    }finally{setAgentReviewBusy(false)}
  }
  async function inspectAgentRecovery(plan){
    if(!chatId||!plan?.planId)return;
    const targetChat=chatId;
    const key=targetChat+':'+plan.planId;
    setAgentReviewBusy(true);
    // Never continue showing a previous successful observation while a
    // new independent read is still pending or has failed.
    setAgentRecoveryByPlan(old=>({...old,[key]:null}));
    try{
      const inspected=await invoke('lu_agent_reconcile_recovery',{
        chatId:targetChat,planId:plan.planId
      });
      if(!inspected||inspected.executionAuthority!=='NOT_AUTHORIZED_FOR_EXECUTION'){
        throw Error('Untrusted recovery authority response');
      }
      setAgentRecoveryByPlan(old=>({...old,[key]:inspected}));
      if(currentChat.current===targetChat){
        setActivity('Recovery evidence inspected only. No tool, restart or model continuation was authorized.');
      }
    }catch(error){
      if(currentChat.current===targetChat){
        setError('Independent agent recovery inspection incomplete: '+String(error)+
          '. Do not infer worker termination or retry a step.');
      }
    }finally{setAgentReviewBusy(false)}
  }
  async function inspectHistoricalRecoveryProvenance(targetChat=chatId){
    if(!targetChat||recoveryProvenanceBusy)return;
    setRecoveryProvenanceBusy(true);
    // Clear an old successful observation BEFORE every new read. A failure
    // must not leave earlier, possibly obsolete evidence visible as current.
    setRecoveryProvenanceByChat(old=>({...old,[targetChat]:null}));
    try{
      const view=await invoke('lu_agent_recovery_provenance',{chatId:targetChat});
      if(!view||view.schema!=='lu-memory-loop02c-operator-provenance-v1'||
          view.executionAuthority!=='NOT_AUTHORIZED_FOR_EXECUTION'||
          view.currentSourceFreshness!=='NOT_RECHECKED'||
          view.priorGrantReusable!==false||
          view.automaticRetryAllowed!==false||
          view.historicalEffectMayBeReplayed!==false||
          view.modelMayOverrideHostSafeDecision!==false||
          !Array.isArray(view.turns)){
        throw Error('Native source-bound recovery evidence was not accepted');
      }
      setRecoveryProvenanceByChat(old=>({...old,[targetChat]:view}));
      if(currentChat.current===targetChat){
        setActivity('Original-source recovery evidence inspected. No native operation or model continuation was authorized.');
      }
    }catch(error){
      if(currentChat.current===targetChat){
        setError('Historical recovery evidence unavailable: '+String(error)+
          '. No historical source, OS termination or tool permission may be inferred.');
      }
    }finally{
      setRecoveryProvenanceBusy(false);
    }
  }
  async function stageHumanAgentReplan(plan){
    if(!chatId||!plan?.revisionSha256||!plan?.planId)return;
    const targetChat=chatId;
    setAgentReviewBusy(true);
    try{
      const outline=replanOutlineByPlan[plan.planId] ??
        humanReplanDefaultOutline(plan.steps);
      const steps=parseHumanReplanOutline(outline,plan.steps);
      const reason=replanReasonByPlan[plan.planId]||'CHANGE_APPROACH';
      if(!REPLAN_REASONS.includes(reason))throw Error('Unknown replan reason');
      const child=await invoke('lu_agent_stage_replan',{
        chatId:targetChat,planId:plan.planId,revisionSha:plan.revisionSha256,
        reason,steps
      });
      if(child===null)return; // Native OS dialog cancelled.
      if(typeof child!=='string'||!child.startsWith('p-')){
        throw Error('Invalid new replan identity');
      }
      await refreshToolActivity(targetChat);
      if(currentChat.current===targetChat){
        setActivity('Replacement draft '+child+' recorded with immutable original-plan lineage. Review it separately; original uncertain processes and approvals were NOT inherited.');
      }
    }catch(error){
      if(currentChat.current===targetChat){
        setError('Replan not created: '+String(error));
      }
      await refreshToolActivity(targetChat).catch(()=>{});
    }finally{setAgentReviewBusy(false)}
  }
  async function prepareFreshAgentContinuation(plan,step){
    if(!chatId||!plan?.revisionSha256||
       !Number.isSafeInteger(step?.stepIndex))return;
    const targetChat=chatId;
    setAgentReviewBusy(true);
    try{
      const child=await invoke('lu_agent_prepare_next_step',{
        chatId:targetChat,planId:plan.planId,
        revisionSha:plan.revisionSha256,stepIndex:step.stepIndex
      });
      if(child===null)return;
      if(typeof child!=='string'||!child.startsWith('p-')){
        throw Error('Invalid fresh child-plan reference');
      }
      await refreshToolActivity(targetChat);
      if(currentChat.current===targetChat){
        setActivity('Fresh child plan '+child+' prepared. Review its separate plan revision, then authorize its one reviewed operation. No old permission was reused.');
      }
    }catch(error){
      if(currentChat.current===targetChat){
        setError('Fresh continuation was not prepared: '+String(error));
      }
      await refreshToolActivity(targetChat).catch(()=>{});
    }finally{setAgentReviewBusy(false)}
  }
  async function runReviewedAgentStep(plan,step){
    if(!chatId||!runtime.sessionId||!plan?.revisionSha256||
       !Number.isSafeInteger(step?.stepIndex))return;
    const targetChat=chatId;
    const key=plan.planId+':'+step.stepIndex;
    const operation=agentStepOpByKey[key]||'git_head';
    setAgentReviewBusy(true);
    try{
      const answer=await invoke('lu_agent_run_reviewed_step',{
        sessionId:runtime.sessionId,chatId:targetChat,
        planId:plan.planId,revisionSha:plan.revisionSha256,
        stepIndex:step.stepIndex,operation
      });
      if(answer===null)return; // Native picker or exact consent cancelled.
      if(typeof answer!=='string')throw Error('Invalid reviewed step response');
      await refreshToolActivity(targetChat);
      if(currentChat.current===targetChat){
        const latest=await loadLatestUiHistory(targetChat).catch(()=>null);
        if(latest)setHistory(latest);
        setActivity('Lu checked one independent step result. New authority is required for another step.');
      }
    }catch(error){
      if(currentChat.current===targetChat){
        setError('Reviewed step not accepted as independently verified: '+String(error)+
          '. If a worker ran, inspect its receipts before retrying.');
      }
      await refreshToolActivity(targetChat).catch(()=>{});
    }finally{setAgentReviewBusy(false)}
  }
  async function cancelAgentPlan(plan){
    if(!chatId||!plan?.planId)return;
    setAgentReviewBusy(true);
    try{
      const acted=await invoke('lu_agent_cancel_plan',{
        chatId,planId:plan.planId
      });
      if(acted)await refreshToolActivity(chatId);
    }catch(error){
      setError('Plan interruption not recorded: '+String(error));
    }finally{setAgentReviewBusy(false)}
  }
  async function setChatMessageReaction(entry,emoji){
    if(!chatId||!Number.isSafeInteger(entry?.seq))return;
    const selected=messageDecorationsByChat[chatId]?.reactionsBySeq?.[entry.seq];
    setMessageDecorationBusy(true);
    try{
      const row=await invoke('lu_set_message_reaction',{
        chatId,seq:entry.seq,emoji:selected===emoji?null:emoji
      });
      const id=chatId;
      setMessageDecorationsByChat(prev=>{
        const prior=prev[id]||{appearance:DEFAULT_APPEARANCE,reactionsBySeq:{}};
        const reactionsBySeq={...prior.reactionsBySeq};
        if(row)reactionsBySeq[entry.seq]=row.emoji;
        else delete reactionsBySeq[entry.seq];
        return {...prev,[id]:{...prior,reactionsBySeq}};
      });
    }catch(error){
      setError('Message reaction not saved: '+String(error));
    }finally{setMessageDecorationBusy(false)}
  }
  async function saveChatAppearance(next){
    if(!chatId)return;
    const id=chatId;
    setMessageDecorationBusy(true);
    try{
      const appearance=await invoke('lu_set_chat_appearance',{
        chatId:id,appearance:safeAppearance(next)
      });
      setMessageDecorationsByChat(prev=>{
        const previous=prev[id]||{reactionsBySeq:{}};
        return {...prev,[id]:{...previous,appearance:safeAppearance(appearance)}};
      });
    }catch(error){
      setError('Chat appearance not saved: '+String(error));
    }finally{setMessageDecorationBusy(false)}
  }
  async function copySavedMessage(entry){
    try{await navigator.clipboard.writeText(entry.text)}
    catch(error){setError('Could not copy this message: '+String(error))}
  }
  function reuseSavedUserPrompt(entry){
    if(entry.role!=='user')return;
    // This only populates the editable composer. It DOES NOT submit,
    // regenerate an effect, or transfer a previous turn's authority.
    setPrompt(entry.text);
    composerRef.current?.focus();
  }
  function quoteSavedMessage(entry){
    const excerpt=entry.text.slice(0,1200)
      .split('\n').map(line=>'> '+line).join('\n');
    setPrompt(prev=>(prev?prev+'\n\n':'')+excerpt+'\n\n');
    composerRef.current?.focus();
  }
  useEffect(()=>{
    const id=chatId;
    if(!id)return;
    let active=true;
    const seqs=history.filter(entry=>Number.isSafeInteger(entry.seq)&&entry.seq>0)
      .map(entry=>entry.seq);
    const windows=[];
    for(let start=0;start<seqs.length;start+=64){
      windows.push(seqs.slice(start,start+64));
    }
    if(!windows.length)windows.push([]);
    // Each host query is strictly <=64 immutable visible message IDs.
    // Pagination can keep more than 64 rows in hot memory; merge decoration
    // windows so older reactions don't vanish when a newer page is loaded.
    Promise.all(windows.map(batch=>invoke('lu_message_decorations',{
      chatId:id,seqs:batch
    }))).then(pages=>{
      if(!active)return;
      const reactionsBySeq=Object.assign({},...pages.map(
        page=>visibleReactionBySeq(page?.reactions)
      ));
      setMessageDecorationsByChat(prev=>({
        ...prev,[id]:{
          appearance:safeAppearance(pages[0]?.appearance),
          reactionsBySeq
        }
      }));
    }).catch(error=>{
      if(active&&currentChat.current===id){
        setError('Chat decoration history unavailable: '+String(error));
      }
    });
    return ()=>{active=false};
  },[chatId,history]);
  // Display-only revision projection over the same bounded original ANSF
  // history window. Never replaces original history or model context.
  useEffect(()=>{
    const id=chatId;
    if(!id)return;
    const seqs=history.filter(x=>
      Number.isSafeInteger(x.seq)&&x.seq>0&&
      (x.role==='user'||x.role==='assistant')
    ).map(x=>x.seq);
    const slices=[];
    for(let start=0;start<seqs.length;start+=64){
      slices.push(seqs.slice(start,start+64));
    }
    if(!slices.length)slices.push([]);
    let active=true;
    Promise.all(slices.map(batch=>invoke('lu_message_revision_page',{
      chatId:id,seqs:batch
    }))).then(pages=>{
      if(!active)return;
      const all=pages.flatMap(page=>page?.entries||[]);
      const pending=Math.max(0,...pages.map(page=>page?.pendingReplyLinks||0));
      setRevisionHistoryByChat(prev=>({
        ...prev,[id]:revisionsBySequence(all)
      }));
      setRevisionPendingByChat(prev=>({...prev,[id]:pending}));
    }).catch(error=>{
      if(active&&currentChat.current===id){
        setError('Message revision history unavailable: '+String(error));
      }
    });
    return ()=>{active=false};
  },[chatId,history]);
  async function editSavedMessage(){
    const editor=messageEditor;
    if(!chatId||!editor||revisionActionBusy)return;
    setRevisionActionBusy(true);
    try{
      const row=await invoke('lu_edit_message_display',{
        chatId,seq:editor.seq,expectedVersion:editor.version,
        expectedSha:editor.sha,text:editor.text
      });
      setRevisionHistoryByChat(prev=>({
        ...prev,[chatId]:{
          ...(prev[chatId]||{}),[row.seq]:row
        }
      }));
      setMessageEditor(null);
    }catch(error){
      setError('Display correction not saved: '+String(error));
    }finally{setRevisionActionBusy(false)}
  }
  async function postLinkedHumanReply(){
    const target=linkedReplyTarget;
    const id=chatId;
    if(!id||!target||!linkedReplyText.trim()||revisionActionBusy)return;
    setRevisionActionBusy(true);
    try{
      const receipt=await invoke('lu_post_linked_reply',{
        chatId:id,parentSeq:target.seq,parentVersion:target.version,
        parentSha:target.sha,text:linkedReplyText
      });
      if(!receipt||receipt.status!=='LINKED_ORIGINAL_HUMAN_TURN_NO_MODEL_EXECUTION'){
        throw Error('Original reply linkage not confirmed');
      }
      setLinkedReplyTarget(null);
      setLinkedReplyText('');
      await refreshChats();
      if(currentChat.current===id){
        const entries=await loadLatestUiHistory(id);
        if(currentChat.current===id)setHistory(entries);
      }
      setActivity('Posted a linked human reply; no model/tool execution.');
    }catch(error){
      setError('Reply outcome uncertain: '+String(error)+
        ' Inspect history and reconcile; do not repost automatically.');
    }finally{setRevisionActionBusy(false)}
  }
  async function reconcileExistingHumanReplies(){
    if(!chatId||revisionActionBusy)return;
    const id=chatId;
    setRevisionActionBusy(true);
    try{
      const linked=await invoke('lu_reconcile_message_replies',{chatId:id});
      setActivity('Reconciled '+linked+' already-saved reply link(s); no messages resent.');
      if(currentChat.current===id){
        const entries=await loadLatestUiHistory(id);
        if(currentChat.current===id)setHistory(entries);
      }
    }catch(error){
      setError('Reply reconciliation failed: '+String(error));
    }finally{setRevisionActionBusy(false)}
  }
  useEffect(()=>{
    setMessageReplay(null);
    setMessageEditor(null);
    setMessageProvenance(null);
    setLinkedReplyTarget(null);
    setLinkedReplyText('');
  },[chatId]);
  useEffect(()=>{
    if(!messageEditor&&!messageProvenance&&!linkedReplyTarget)return;
    const onEscape=event=>{
      if(event.key==='Escape'){
        setMessageEditor(null);
        setMessageProvenance(null);
        setLinkedReplyTarget(null);
      }
    };
    window.addEventListener('keydown',onEscape);
    return ()=>window.removeEventListener('keydown',onEscape);
  },[messageEditor,messageProvenance,linkedReplyTarget]);
  useEffect(()=>{
    if(!messageReplay)return;
    const closeOnEscape=event=>{
      if(event.key==='Escape')setMessageReplay(null);
    };
    window.addEventListener('keydown',closeOnEscape);
    return ()=>window.removeEventListener('keydown',closeOnEscape);
  },[messageReplay]);
  const chatAppearance=safeAppearance(
    messageDecorationsByChat[chatId]?.appearance
  );
  const chatReactions=messageDecorationsByChat[chatId]?.reactionsBySeq||{};
  const displayRevisions=revisionHistoryByChat[chatId]||{};
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
        case 'extract_archive_batch':await reviewArchiveBatch();break;
        case 'split_archive':await runP08Multipart('split');break;
        case 'create_document':
          if(approved.documentDraft) setDocumentDraft({
            format:approved.documentDraft.format,
            title:approved.documentDraft.title,
            content:approved.documentDraft.content
          });
          setDocumentComposerOpen(true);break;
        case 'revise_office':
          if(approved.officePatch)setOfficePatch(approved.officePatch);
          setOfficeRevisionOpen(true);break;
        case 'batch_office':
          if(approved.officeBatch)setOfficeBatchDraft(approved.officeBatch);
          setOfficeBatchOpen(true);break;
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
    const usePlanDraft=planDraftNextSend;
    const useBrowserResearch=browserResearchNextSend;
    const useReviewedApp=reviewedAppNextSend;
    // Explicit human folder requests may enter existing native read-only
    // workspace mode. The path text is never authority; user selects an
    // actual directory through Lu's fresh native OS folder picker.
    // This flag is only the historical explicit user opt-in. Ordinary
    // user requests are classified by the native lu_generate host, which
    // prioritizes approved Git, Reader, media and HTTPS Browser routes
    // before workspace. Renderer heuristics never choose a tool scope.
    const useWorkspaceTools=workspaceToolsNextSend;
    if([Boolean(useWorkspaceTools),Boolean(useBrowserResearch),Boolean(useReviewedApp),Boolean(usePlanDraft)].filter(Boolean).length>1){
      setError('Choose only one native workspace, reviewed app or Browser mode for this turn.');return;
    }
    if(usePlanDraft&&(images.length||textFiles.length||input.length>8192)){
      setError('Draft plan mode requires a text-only original human request, at most 8192 characters.');return;
    }
    if(usePlanDraft&&!m2Context?.goalId){
      setError('Set a saved ANSF goal before asking Lu to draft a plan.');return;
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
    if (useReviewedApp && (images.length || textFiles.length || input.length>8192)){
      setError('Reviewed application tools require a text-only question no longer than 8192 characters.');return;
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
    if (usePlanDraft) setPlanDraftNextSend(false);
    if (useBrowserResearch) setBrowserResearchNextSend(false);
    if (useReviewedApp) setReviewedAppNextSend(null);
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
      const output = useReviewedApp
        ? await invoke('lu_reviewed_app_ask', {
            sessionId:runtime.sessionId,chatId:activeId,prompt:input,
            mode:useReviewedApp
          })
        : useBrowserResearch
          ? await invoke('lu_generate_browser', {
              sessionId:runtime.sessionId,chatId:activeId,
              prompt:input,origins:browserOrigins,maxNewTokens:512
            })
          : await invoke('lu_generate', {
            sessionId: runtime.sessionId, chatId: activeId,
            prompt: submitted, imageRefs: refs, maxNewTokens: 512,
            readOnlyWorkspace: useWorkspaceTools,
            planOnly: usePlanDraft
          });
      if (output===null && useReviewedApp) {
        // Native picker cancellation creates no user turn or permission.
        cancelled = true;
        if (currentChat.current === activeId) {
          draftText.current=input;
          pendingDrafts.current.set(activeId,{prompt:input,images:[],files:[]});
          setPrompt(input);
        }
      } else {
        if (typeof output !== 'string') throw Error('Lu did not return a text response');
        completed = true;
      }
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
      if (usePlanDraft) await refreshToolActivity(activeId).catch(error=>{
        if (currentChat.current===activeId)setError(String(error));
      });
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
        {/* Background receipts remain in the conversation on demand; no dashboard button. */}
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
          {working && activity.startsWith('Loading') && <span className="busyHint" role="status" aria-live="polite"><span className="spinner" aria-hidden="true"></span> Loading model · {modelLoadTrace?.loading&&modelLoadTrace.events?.length ? modelLoadTrace.events[modelLoadTrace.events.length-1].phase.replaceAll('_',' ') : 'Reading model and initializing backend'} · {modelMemorySample?.workingSetRamBytes!=null
            ? 'RAM process '+(modelMemorySample.workingSetRamBytes/1073741824).toFixed(2)+' GiB'
            : 'RAM unmeasured'} · {modelMemorySample?.dedicatedGpuBytes!=null
            ? 'GPU process '+(modelMemorySample.dedicatedGpuBytes/1073741824).toFixed(2)+' GiB'
            : 'GPU unmeasured'} · Tensor loading % unavailable</span>}
          <div className="modelSwitcherWrap" ref={modelMenuRef}>
            <button className="modelSwitcher" aria-label="Choose a model" onClick={() => setModelMenuOpen(v => !v)} aria-expanded={modelMenuOpen} title="Quick model selection">
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
                <button className="subtleButton" onClick={() => {setModelMenuOpen(false);void invoke('lu_open_aux_window',{kind:'model-library'}).catch(error=>setError(String(error)));}}>Manage models</button>
              </div>
              {loaded && <button className="subtleButton full" disabled={busy} onClick={() => {setModelMenuOpen(false);unloadModel();}}>⏏ Unload active model</button>}
            </div>}
          </div>
          {loaded && <button className="quickEject" disabled={busy} title="Unload active model" onClick={unloadModel}>⏏</button>}
          <button type="button" className="subtleButton" title="Open independent ANSF history"
            disabled={!chatId} aria-expanded={historyPaneOpen}
            onClick={()=>setHistoryPaneOpen(v=>!v)}>History library</button>
          <button className="quietSettings" title="Settings" aria-label="Settings"
            onClick={()=>void invoke('lu_open_aux_window',{kind:'settings'}).catch(error=>setError(String(error)))}>⚙</button>
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
      <div className="transcriptWrap"><div className="messages" ref={transcriptRef}
        style={{backgroundColor:chatAppearance.background,color:textForBackground(chatAppearance.background)}}
        onScroll={onTranscriptScroll} role="log" aria-live="off" aria-label="Conversation history, cursor paginated">
        {historyWindow.hasOlder&&<button type="button" className="subtleButton"
          onClick={()=>void loadOlderUiHistory()}>Load older ANSF page</button>}

        {!history.length && <div className="welcome"><div className="welcomeSymbol">L</div>
          <h2>What can I help you build?</h2>
          <p>Local conversations are saved separately. Start a new chat or choose one from your history.</p></div>}
        {displayedHistory.map((entry,i) => {
          const revision=displayRevisions[entry.seq];
          const displayText=revision?.currentText??entry.text;
          return <div key={entry.seq??i}
          data-ansf-seq={entry.seq??undefined}
          className={'message ' + (entry.role === 'user' ? 'user' : 'assistant')}
          style={{backgroundColor:entry.role==='user'?chatAppearance.userBubble:chatAppearance.assistantBubble,
            color:textForBackground(entry.role==='user'?chatAppearance.userBubble:chatAppearance.assistantBubble)}}>
          <span className="messageRole">{entry.role === 'user' ? 'You' : 'Lu'}{entry.cancelled ? ' · stopped' : entry.interrupted ? ' · interrupted' : ''}</span>
          {revision?.replyTo&&<button type="button" className="messageReplyReference"
            onClick={()=>setMessageProvenance({entry,revision})}
            aria-label={'Inspect reply provenance for message '+entry.seq}>
            ↳ Reply to original #{revision.replyTo.parentSeq},
            version {revision.replyTo.pinnedParentVersion}:
            {' '+revision.replyTo.pinnedParentExcerpt}
          </button>}
          <MessageText text={displayText} role={entry.role}/>
          {revision?.currentVersion>0&&<small className="messageEditedNotice">
            Human-edited display · revision {revision.currentVersion}.
            Original model input and tool history unchanged.
          </small>}
          {Number.isSafeInteger(entry.seq)&&entry.seq>0&&
            <div className="messageActions" role="group" aria-label={'Actions for saved message '+entry.seq}>
              <button type="button"
                onClick={()=>void copySavedMessage({...entry,text:displayText})}
                aria-label="Copy displayed message">Copy</button>
              <button type="button" onClick={()=>setMessageReplay({
                chatId,seq:entry.seq,role:entry.role,text:entry.text
              })} aria-label="Replay original saved message">Replay</button>
              {entry.role==='user'&&<button type="button"
                onClick={()=>reuseSavedUserPrompt(entry)} aria-label="Reuse saved prompt without sending">
                Reuse prompt
              </button>}
              <button type="button" onClick={()=>quoteSavedMessage({
                ...entry,text:displayText
              })} aria-label="Quote displayed message in composer">Quote</button>
              {canEdit(entry)&&revision&&<>
                <button type="button"
                  onClick={()=>setMessageEditor({
                    seq:entry.seq,role:entry.role,version:revision.currentVersion,
                    sha:revision.currentSha256,text:revision.currentText,
                    original:revision.originalText
                  })} aria-label="Edit displayed message without changing original">
                  Edit display
                </button>
                <button type="button"
                  onClick={()=>setMessageProvenance({entry,revision})}
                  aria-label="View original, revisions and provenance">
                  Versions / source
                </button>
                <button type="button" onClick={()=>{
                  const target=draftReplyTarget(entry,revision);
                  if(target){setLinkedReplyTarget({...target,chatId});setLinkedReplyText('')}
                }} aria-label="Reply with a new linked human message">
                  Reply
                </button>
              </>}
              <div className="messageReactions" role="group" aria-label="React to this message">
                <button type="button" className="reactionTrigger"
                  aria-label="Open emoji reactions" aria-expanded={reactionPickerSeq===entry.seq}
                  onClick={()=>setReactionPickerSeq(v=>v===entry.seq?null:entry.seq)}>
                  ☺ {chatReactions[entry.seq]||''}
                </button>
                {reactionPickerSeq===entry.seq&&<div className="reactionPicker" role="group" aria-label="Choose a reaction below this message">
                  {MESSAGE_REACTIONS.map(emoji=><button type="button" key={emoji}
                    disabled={messageDecorationBusy} aria-pressed={chatReactions[entry.seq]===emoji}
                    aria-label={'React '+emoji}
                    onClick={()=>{void setChatMessageReaction(entry,emoji);setReactionPickerSeq(null);}}>
                    {emoji}
                  </button>)}
                </div>}
              </div>
            </div>}
        </div>;
        })}
        {(revisionPendingByChat[chatId]||0)>0&&
          <div className="replyRecoveryNotice" role="status">
            {revisionPendingByChat[chatId]} reply link(s) need examination.
            Reconciliation checks only already-saved human messages.
            <button type="button" disabled={revisionActionBusy}
              onClick={()=>void reconcileExistingHumanReplies()}>
              Reconcile saved reply links
            </button>
          </div>}
        {historyWindow.hasNewer&&<button type="button" className="subtleButton"
          onClick={()=>void loadNewerUiHistory()}>Load newer ANSF page</button>}
        {visibleJob && <div className="message assistant streaming"><span className="messageRole" role="status" aria-live="polite" aria-atomic="true">
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
      {messageReplay?.chatId===chatId&&<div className="messageReplayBackdrop"
        role="presentation" onClick={()=>setMessageReplay(null)}>
        <section className="messageReplayDialog" role="dialog" aria-modal="true"
          aria-label="Replay saved conversation message"
          onClick={event=>event.stopPropagation()}>
          <header><strong>Replay saved message · {messageReplay.role==='user'?'You':'Lu'}</strong>
            <button type="button" autoFocus onClick={()=>setMessageReplay(null)}>Close</button></header>
          <small>This is an exact saved display replay; it never reruns the AI, calls tools,
            or restores earlier authority.</small>
          <MessageText text={messageReplay.text} role={messageReplay.role}/>
          <div className="settingsActions">
            <button type="button" onClick={()=>void copySavedMessage(messageReplay)}>
              Copy original
            </button>
            <button type="button" onClick={()=>setMessageReplay(null)}>Done</button>
          </div>
        </section>
      </div>}
      {messageEditor&&chatId&&<div className="messageReplayBackdrop"
        role="presentation" onClick={()=>setMessageEditor(null)}>
        <section className="messageReplayDialog" role="dialog" aria-modal="true"
          aria-label="Edit message display revision"
          onClick={event=>event.stopPropagation()}>
          <header><strong>Correct displayed message #{messageEditor.seq}</strong>
            <button type="button" onClick={()=>setMessageEditor(null)}>Close</button>
          </header>
          <small>Human display correction only. The immutable ANSF original,
            previous model input, and all earlier tool permissions remain unchanged.</small>
          <label className="messageRevisionEditor">Corrected text
            <textarea autoFocus rows={8} maxLength={16384}
              value={messageEditor.text}
              onChange={event=>setMessageEditor(old=>({
                ...old,text:event.target.value
              }))}/>
          </label>
          <details className="originalMessageDetails">
            <summary>View immutable original for comparison</summary>
            <pre>{messageEditor.original}</pre>
          </details>
          <div className="settingsActions">
            <button type="button" disabled={revisionActionBusy||
              !messageEditor.text.trim()||
              messageEditor.text===(
                displayRevisions[messageEditor.seq]?.currentText
              )}
              onClick={()=>void editSavedMessage()}>
              Save new display revision
            </button>
            <button type="button" onClick={()=>setMessageEditor(null)}>Cancel</button>
          </div>
        </section>
      </div>}
      {messageProvenance&&chatId&&<div className="messageReplayBackdrop"
        role="presentation" onClick={()=>setMessageProvenance(null)}>
        <section className="messageReplayDialog" role="dialog" aria-modal="true"
          aria-label="Immutable message provenance and edit versions"
          onClick={event=>event.stopPropagation()}>
          <header><strong>Source and revisions · message #{messageProvenance.entry.seq}</strong>
            <button type="button" autoFocus
              onClick={()=>setMessageProvenance(null)}>Close</button>
          </header>
          <small>Authenticated ANSF originals are retained even after a display correction.
            A human correction is not a new model answer or tool-execution receipt.</small>
          <div className="settingsActions">
            <button type="button" disabled={revisionActionBusy||!
              correctedMessageFollowUp(messageProvenance.entry,messageProvenance.revision)}
              onClick={()=>{
                const prepared=correctedMessageFollowUp(
                  messageProvenance.entry,messageProvenance.revision);
                if(!prepared)return;
                setLinkedReplyTarget({...prepared.target,chatId});
                setLinkedReplyText(prepared.draft);
                setMessageProvenance(null);
              }}>
              Prepare human follow-up from correction
            </button>
          </div>
          <small>This only opens a NEW human reply draft pinned to the current corrected
            version. Posting it does not trigger inference or inherit earlier approval.</small>
          <div className="messageProvenanceVersions">
            <section>
              <strong>Original ANSF text · version 0</strong>
              <small>SHA-256: {messageProvenance.revision.originalSha256}</small>
              <pre>{messageProvenance.revision.originalText}</pre>
            </section>
            {messageProvenance.revision.versions.map(v=>
              <section key={v.version}>
                <strong>Version {v.version} · human display correction</strong>
                <small>SHA-256: {v.sha256}</small>
                <pre>{v.text}</pre>
              </section>
            )}
            {messageProvenance.revision.replyTo&&<section>
              <strong>Linked reply provenance</strong>
              <small>Parent ANSF message: #{messageProvenance.revision.replyTo.parentSeq}
                {' '}· pinned version {messageProvenance.revision.replyTo.pinnedParentVersion}
              </small>
              <small>Parent SHA-256:
                {' '+messageProvenance.revision.replyTo.pinnedParentSha256}</small>
              <pre>{messageProvenance.revision.replyTo.pinnedParentExcerpt}</pre>
              <small>The parent link never changes when the parent is edited later.</small>
            </section>}
          </div>
        </section>
      </div>}
      {linkedReplyTarget?.chatId===chatId&&<div className="messageReplayBackdrop"
        role="presentation" onClick={()=>setLinkedReplyTarget(null)}>
        <section className="messageReplayDialog" role="dialog" aria-modal="true"
          aria-label="Write a linked human reply" onClick={event=>event.stopPropagation()}>
          <header><strong>Reply to saved message #{linkedReplyTarget.seq}</strong>
            <button type="button" onClick={()=>setLinkedReplyTarget(null)}>Close</button>
          </header>
          <small>Target display version {linkedReplyTarget.version} ·
            pinned SHA-256 {linkedReplyTarget.sha}</small>
          <blockquote className="messagePinnedQuote">
            {linkedReplyTarget.displayText}
          </blockquote>
          <label className="messageRevisionEditor">New human reply
            <textarea autoFocus rows={6} maxLength={16384}
              value={linkedReplyText} onChange={event=>setLinkedReplyText(event.target.value)}
              placeholder="Write a reply linked to this specific message and version"/>
          </label>
          <small>Posting writes a new saved human message and a provenance link.
            It does not send this text to the AI, call tools, or renew old permissions.</small>
          <div className="settingsActions">
            <button type="button" disabled={revisionActionBusy||!linkedReplyText.trim()}
              onClick={()=>void postLinkedHumanReply()}>
              Post linked human reply
            </button>
            <button type="button" onClick={()=>setLinkedReplyTarget(null)}>Cancel</button>
          </div>
        </section>
      </div>}
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
          <small>Host-recorded Reader, Workspace, Git and reviewed-app tool-turn evidence.
            Model answers are not OS receipts. E1-P10 independently compares
            approved Git HEAD/branch and PCM WAV
            facts with the source itself, not a second Git/FFprobe invocation.
            Other operations remain unverified. An interrupted worker is
            not proven terminated by its journal state. Viewing never restores
            permission or replays an operation.</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={toolHistoryBusy}
              onClick={()=>void refreshToolActivity(chatId)}>
              {toolHistoryBusy?'Refreshing…':'Refresh tool activity'}
            </button>
            <button type="button" className="subtleButton"
              onClick={()=>setToolActivityOpen(false)}>Close</button>
          </div>

          <section className="readProposalCard" role="region"
            aria-label="Reviewed memory projection recovery">
            <strong>Reviewed memory projection recovery</strong>
            <small>At startup Lu reconstructs only historical derived memory while
              holding its exclusive profile lease. No source is re-executed,
              current file freshness is not established, and no permission to
              retry or resume is created.</small>
            {(reviewedMemoryRecoveryByChat[chatId]||[]).length===0
              ? <small>No interrupted projection was reported for this chat at startup.</small>
              : (reviewedMemoryRecoveryByChat[chatId]||[]).map((entry,index)=>{
                  const detail=explainMemoryProjectionRecovery(entry);
                  return <div className="readProposalCard" key={entry.originalTurnId+index}
                    role="status">
                    <strong>{detail.title}</strong>
                    <small>{entry.operation} · Original turn {entry.originalTurnId}</small>
                    <small>{detail.description}</small>
                    <small>{entry.explanation}</small>
                    <small>Execution: {entry.executionAuthority}; automatic retry:
                      {entry.retryAllowed?'allowed':'denied'}.</small>
                  </div>;
                })}
          </section>
          <section className="readProposalCard" role="region"
            aria-label="Original-source recovery provenance">
            <strong>Original-source recovery provenance</strong>
            <small>This view is computed by Lu's native host from authenticated
              human turns, original task/effect journals, independent source
              checks and OS custody witnesses. It is not based on what the
              loaded AI claims. Results are historical snapshots, never proof
              that files or processes are unchanged now.</small>
            <div className="settingsActions">
              <button type="button" className="subtleButton"
                disabled={recoveryProvenanceBusy}
                onClick={()=>void inspectHistoricalRecoveryProvenance(chatId)}>
                {recoveryProvenanceBusy?'Inspecting…':'Inspect original evidence'}
              </button>
            </div>
            {recoveryProvenanceByChat[chatId]&&
              <div className="recoveryProvenance" role="status"
                aria-label="Host historical recovery report">
                <strong>Host report: {recoveryProvenanceByChat[chatId].status.replaceAll('_',' ')}</strong>
                <small>Required next action:
                  {' '+recoveryProvenanceByChat[chatId].hostSafeNextAction.replaceAll('_',' ')}</small>
                <small>{recoveryProvenanceByChat[chatId].hostSafeNextExplanation}</small>
                <small>Current source freshness: NOT RECHECKED.
                  Execution authority: NOT AUTHORIZED. Inspect again after
                  any source, process or model change; this is not a live watch.</small>
                {recoveryProvenanceByChat[chatId].olderHostTurnsNotInspected&&
                  <small className="recoveryProvenanceWarning">Older authenticated
                    human turns were not all inspected. This bounded view
                    cannot clear their historical effects.</small>}
                {(recoveryProvenanceByChat[chatId].turns||[]).length===0&&
                  <small>No original reviewed effects could be established in
                    this bounded human-turn window. Absence of evidence is
                    not evidence of safe completion.</small>}
                {(recoveryProvenanceByChat[chatId].turns||[]).map(turn=>
                  <div className="recoveryProvenanceTurn" key={turn.originalHumanTurnId}>
                    <strong>Original human turn: {turn.originalHumanTurnId}</strong>
                    <small>Original source bound: {turn.originBound?'yes':'NO'}
                      {' '}· Journal: {turn.journalStatus.replaceAll('_',' ')}
                      {' '}· Reviewed operations: {turn.reviewedOperationCount}</small>
                    <small>Turn action: {turn.hostSafeNextAction.replaceAll('_',' ')}</small>
                    {turn.additionalOperationsNotRead&&
                      <small className="recoveryProvenanceWarning">Additional
                        original effect operations were not read. Reconcile
                        complete original history before acting.</small>}
                    {(turn.operations||[]).map((operation,index)=>
                      <div className="recoveryProvenanceOperation"
                        key={turn.originalHumanTurnId+':'+index}>
                        <strong>{operation.operation}</strong>
                        <small>Independent source verdict:
                          {' '+operation.independentOracleAtObservation.replaceAll('_',' ')}</small>
                        <small>Original effect: {operation.historicalEffectState}
                          {' '}· Model call linked: {operation.originalModelCallLinked?'yes':'NO'}
                          {' '}· Worker completed: {operation.completedWorkerTaskLinked?'yes':'NO'}</small>
                        <small>Descendant custody: {operation.descendantCustody.replaceAll('_',' ')}
                          {' '}· Current freshness: NOT RECHECKED</small>
                        {operation.independentlyObservedSha256&&
                          <small className="recoveryProvenanceDigest">Historical
                            independent source SHA-256: {operation.independentlyObservedSha256}</small>}
                        <small>Next action: {operation.hostSafeNextAction.replaceAll('_',' ')}</small>
                      </div>)}
                  </div>)}
                <small>Historical grants cannot be reused. Automatic retry,
                  worker resumption and model override are prohibited.
                  Any new effect requires native source verification and
                  separate fresh human authorization.</small>
              </div>}
          </section>
          <section className="readProposalCard" role="region"
            aria-label="Durable agent planning and recovery">
            <strong>Agent work plans — original conversation</strong>
            <small>Read-only, host-owned step history. Approval applies only to a
              plan revision, not tool execution. A previous checked receipt is
              historical evidence, not proof of current source or worker state.</small>
            {(agentProgressByChat[chatId]||[]).length===0 &&
              <small>No durable agent plans are linked to this conversation yet.
                Ordinary chat and reviewed tools remain available through their
                existing separate authorization.</small>}
            {(agentProgressByChat[chatId]||[]).map(plan=>
              <div className="readProposalCard" key={plan.planId}
                role="group" aria-label={'Agent plan '+plan.planId}>
                <strong>Plan {plan.planId.slice(0,14)} · {plan.planState.replaceAll('_',' ')}</strong>
                <small>Revision {plan.latestRevision} · Human review:
                  {' '+plan.revisionReview.replaceAll('_',' ')} · Supervisor:
                  {' '+plan.supervisorState.replaceAll('_',' ')}</small>
                <small>Original saved human turn: {plan.originalHumanTurnId}</small>
                <div className="settingsActions">
                  <button type="button" className="subtleButton"
                    disabled={agentReviewBusy}
                    title="Read original plan, task, effect and independent-oracle journals. Never repair or replay."
                    onClick={()=>void inspectAgentRecovery(plan)}>
                    Review independent recovery evidence
                  </button>
                </div>
                {agentRecoveryByPlan[chatId+':'+plan.planId]&&
                  <section className="readProposalCard"
                    aria-label="Independent agent recovery decision">
                    <strong>Recovery inspection (historical, read-only)</strong>
                    <small>Status: {agentRecoveryByPlan[chatId+':'+plan.planId].status}</small>
                    <small>Independently checked task/effect results:
                      {' '+agentRecoveryByPlan[chatId+':'+plan.planId].independentlyObservedSteps}
                      {' '}· Ancestor generations:
                      {' '+agentRecoveryByPlan[chatId+':'+plan.planId].linkedGenerations}</small>
                    {(agentRecoveryByPlan[chatId+':'+plan.planId].steps||[]).map(step=>
                      <small key={step.stepIndex}>Step {step.stepIndex+1}:
                        {' '+step.evidenceStatus}; descendant custody:
                        {' '+step.descendantCustody}</small>
                    )}
                    <small>{agentRecoveryByPlan[chatId+':'+plan.planId].nextAction}</small>
                    <small>No execution authority was granted. This snapshot
                      cannot be used to resume a model or replay an OS effect.</small>
                  </section>}
                {plan.pendingSourceChecks>0 &&
                  <small>Assumptions requiring independent authorized source review:
                    {' '+plan.pendingSourceChecks}. Approval remains unavailable.</small>}
                <div className="settingsActions">
                  {plan.revisionReview==='PENDING_HUMAN_REVIEW'&&plan.revisionSha256&&
                    <>
                      <button type="button" className="subtleButton"
                        disabled={agentReviewBusy||plan.pendingSourceChecks>0}
                        onClick={()=>void reviewAgentPlan(plan,'approve')}>
                        Approve plan only
                      </button>
                      <button type="button" className="subtleButton"
                        disabled={agentReviewBusy}
                        onClick={()=>void reviewAgentPlan(plan,'reject')}>
                        Reject plan
                      </button>
                    </>}
                  {plan.planState!=='CANCEL_REQUESTED' &&
                    <button type="button" className="subtleButton"
                      disabled={agentReviewBusy}
                      onClick={()=>void cancelAgentPlan(plan)}>
                      Record cancellation intent
                    </button>}
                </div>
                <small>Approval, rejection and cancellation require a native
                  confirmation. None launches, resumes or terminates an OS worker.</small>
                {plan.revisionSha256&&
                  <details className="readProposalCard">
                    <summary>Revise this plan with a new human request</summary>
                    <small>Change the ordered steps (1–8) using COLLECT_SOURCE,
                      INSPECT, ANALYZE, INDEPENDENT_CHECK, HUMAN_REVIEW or
                      PRESENT, separated by commas. Each step depends on the
                      preceding step. Unsupported types remain review-only;
                      nothing runs or reuses an earlier grant.</small>
                    <label>Replacement steps
                      <textarea rows={2}
                        value={replanOutlineByPlan[plan.planId] ??
                          humanReplanDefaultOutline(plan.steps)}
                        onChange={e=>setReplanOutlineByPlan(prev=>({
                          ...prev,[plan.planId]:e.target.value
                        }))}/>
                    </label>
                    <label>Reason
                      <select value={replanReasonByPlan[plan.planId]||'CHANGE_APPROACH'}
                        onChange={e=>setReplanReasonByPlan(prev=>({
                          ...prev,[plan.planId]:e.target.value
                        }))}>
                        {REPLAN_REASONS.map(reason=>
                          <option key={reason} value={reason}>
                            {reason.replaceAll('_',' ').toLowerCase()}
                          </option>)}
                      </select>
                    </label>
                    <button type="button" className="subtleButton"
                      disabled={agentReviewBusy||working||chatBusy}
                      title="A native confirmation records a distinct human plan and immutable ancestry; the previous operation remains subject to recovery before new execution."
                      onClick={()=>void stageHumanAgentReplan(plan)}>
                      Create separately reviewed replacement plan
                    </button>
                    <small>A failed or uncertain original worker is not retried.
                      Any replacement execution is blocked until the original
                      process custody and independent outcome are accounted for.</small>
                  </details>}
                {plan.steps.map(step=>
                  <div className="readProposalCard"
                    key={step.stepIndex} role="group"
                    aria-label={'Agent step '+(step.stepIndex+1)}>
                    <strong>Step {step.stepIndex+1}: {step.purpose.replaceAll('_',' ').toLowerCase()}</strong>
                    <small>Expected result: {step.expectedDeliverable.replaceAll('_',' ').toLowerCase()}</small>
                    <small>Dependency: {step.dependsOn===null?
                      'none':'step '+(step.dependsOn+1)} ·
                      {' '+step.scheduleState.replaceAll('_',' ').toLowerCase()}</small>
                    {step.historicalWitnessLinked&&
                      <small>Independent witness reference was recorded.
                        Live source, process custody and receipt must be rechecked.</small>}
                    {plan.continuationPreview?.status==='PREPARE_NEW_CHILD_PLAN_ONLY'&&
                      plan.continuationPreview?.candidateStepIndex===step.stepIndex&&
                      plan.supervisorState==='RELEASED'&&
                      step.scheduleState==='NOT_SCHEDULED'&&
                      <div className="settingsActions">
                        <button type="button" className="subtleButton"
                          disabled={agentReviewBusy||working||chatBusy}
                          title="Native confirmation prepares a new immutable child plan only; another human plan review and separate per-tool native authorization are still required."
                          onClick={()=>void prepareFreshAgentContinuation(plan,step)}>
                          Prepare separate next-step plan
                        </button>
                        <small>Rechecks original ANSF and independent evidence. This does not execute a worker, revive the old owner or reuse a model grant.</small>
                      </div>}
                    {plan.revisionReview==='APPROVED_PLAN_ONLY'&&
                      plan.supervisorState==='NOT_CLAIMED'&&
                      plan.planState!=='CANCEL_REQUESTED'&&
                      plan.planState!=='NEEDS_REVIEW'&&
                      plan.continuationPreview?.status==='MANUAL_ONE_STEP_REVIEW_ONLY'&&
                      plan.continuationPreview?.candidateStepIndex===step.stepIndex&&
                      (step.dependsOn===null||
                        plan.steps[step.dependsOn]?.scheduleState==='HISTORICAL_WITNESS_RECORDED')&&
                      step.purpose==='INDEPENDENT_CHECK'&&
                      step.expectedDeliverable==='INDEPENDENT_WITNESS'&&
                      step.scheduleState==='NOT_SCHEDULED'&&
                      <div className="settingsActions">
                        <label>Reviewed source operation
                          <select value={agentStepOpByKey[plan.planId+':'+step.stepIndex]||'git_head'}
                            onChange={e=>setAgentStepOpByKey(prev=>({
                              ...prev,[plan.planId+':'+step.stepIndex]:e.target.value
                            }))}>
                            <option value="git_head">Git HEAD</option>
                            <option value="git_status">Git tracked status</option>
                            <option value="git_branch">Git current branch</option>
                            <option value="git_history">Git history count</option>
                            <option value="media_facts">FFprobe media facts</option>
                          </select>
                        </label>
                        <button type="button" className="subtleButton"
                          disabled={!loaded||working||chatBusy||agentReviewBusy}
                          title="OS-select a resource and explicitly authorize exactly one inspected plan step. The loaded model uses one existing reviewed Git/FFprobe tool; Lu checks the worker's actual independent outcome."
                          onClick={()=>void runReviewedAgentStep(plan,step)}>
                          Review and run one step
                        </button>
                        <small>A new native permission dialog and selected local source are required.
                          No old plan approval grants execution. Unsupported or uncertain
                          results never complete a step automatically.</small>
                      </div>}
                  </div>
                )}
                <small>Next safe action: {plan.nextSafeAction}</small>
                {plan.continuationPreview&&
                  <section className="readProposalCard"
                    aria-label="Agent next-step preparation and recovery preview">
                    <strong>Next-step preparation (read-only)</strong>
                    <small>Continuation ancestry: {plan.lineageStatus||'not evaluated'}
                      {' '}· {Number.isSafeInteger(plan.lineageGenerations)?plan.lineageGenerations:0} child generations.
                      {plan.lineageParentId?' Parent plan: '+plan.lineageParentId:''}</small>
                    {plan.lineageStatus?.startsWith('STOP_')&&
                      <small>Lineage requires manual reconciliation; no new worker or reused authority.</small>}
                    <small>{plan.continuationPreview.requiredAction}</small>
                    {Number.isSafeInteger(plan.continuationPreview.candidateStepIndex)&&
                      <small>Plan step {plan.continuationPreview.candidateStepIndex+1}
                        {' '}is a candidate for review, not an execution instruction.</small>}
                    <small>Current source independently reverified: no.
                      A historic witness link is not current source correctness.</small>
                    <small>No new host owner, worker, tool permission or automatic next turn
                      is created from this preview.</small>
                  </section>}
                <small>Execution authority: none. No automatic retry or
                  continuation is available from this history view.</small>
              </div>
            )}
          </section>
          {(toolHistoryByChat[chatId]||[]).length===0&&
            <small>No persisted native model/tool events in this conversation.</small>}
          {(toolHistoryByChat[chatId]||[]).map(item=>
            <div key={item.source+':'+item.taskId} className="readProposalCard"
              role="group" aria-label={'Tool audit '+item.taskId}>
              <strong>{item.source} — {explainToolOutcome(item.source,item.state).title}</strong>
              <small>{explainToolOutcome(item.source,item.state).meaning}</small>
              <small>Next safe step: {explainToolOutcome(item.source,item.state).nextStep}</small>
              <small>{item.recordedAt} · Task {item.taskId}</small>
              <small>{item.modelResponses} model responses ·
                {item.toolFeedback} tool results ·
                {item.rejectedEvents} rejections ·
                {item.repairedResponses} structured repairs</small>
              {item.source==='Reviewed app' && <small>
                Reviewed app effect journal: {item.reviewedEffects
                  ? `${item.reviewedEffects.appliedWithReceipt} applied with retained receipt · ${item.reviewedEffects.uncertain} uncertain · ${item.reviewedEffects.failed} failed · ${item.reviewedEffects.executing} executing · ${item.reviewedEffects.prepared} prepared`
                  : 'No effect journal exists; no verified worker outcome can be inferred.'}
              </small>}
              {item.source==='Reviewed app' && <small>
                Independent outcomes at the time of observation: {item.reviewedEffects
                  ? `${item.reviewedEffects.independentlyVerified||0} verified · ${item.reviewedEffects.outcomeUnsupported||0} unsupported · ${item.reviewedEffects.outcomeConflict||0} contradicted`
                  : 'No independently verified outcome evidence'}.
              </small>}
              {item.source==='Reviewed app' && <small>
                Verified means a distinct host-only source check matched the
                recorded application result and an approved-source revision check
                remained stable before and after the supervised worker. It is not
                an atomic filesystem snapshot and does not prove that files will
                remain unchanged or that processes are still terminated.
                APPLIED alone means an execution receipt, not semantic truth.
              </small>}
              {item.source==='Reviewed app' && item.reviewedContinuity && <section
                role="region" aria-label="Reviewed task recovery timeline"
                className="readProposalCard">
                <strong>Original conversation — durable task recovery</strong>
                <small>{explainReviewedContinuation(item.reviewedContinuity.continuation).title}</small>
                <small>{explainReviewedContinuation(item.reviewedContinuity.continuation).explanation}</small>
                <small>{item.reviewedContinuity.explanation}</small>
                <small>Next safe step: {explainReviewedContinuation(item.reviewedContinuity.continuation).action}</small>
                <small>Origin bound: {item.reviewedContinuity.originBound?'yes':'not established'}.
                  Journal: {item.reviewedContinuity.journalState}.
                  This view never resumes a task or grants execution.</small>
                {item.reviewedContinuity.operations.map((op,index)=>
                  <div key={index} className="readProposalCard">
                    <strong>{op.operation}</strong>
                    <small>OS effect: {op.processState}. Independent outcome: {op.outcome}.</small>
                    <small>Original model call: {op.originalModelCallLinked?'linked':'not proven'}.
                      Worker task: {op.workerTaskLinked?'completed in journal':'not proven complete'}.</small>
                    <small>Durable checkpoints: {op.checkpointEvents};
                      last recorded progress: {op.lastCheckpoint==null?'not saved':op.lastCheckpoint+'%'}.</small>
                  </div>
                )}
              </section>}
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
            {/* Legacy specialist controls retained temporarily for compatibility, but no longer mounted. Native tools enter through the original human chat and per-turn OS approvals. */}
            
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
      </div>;
}

// Fixed, host-created native window URLs isolate Settings/Model Library from
// App() chat effects, generation event handlers, queues, model prompts and drafts.
const luWindow=new URLSearchParams(window.location.search).get('lu-window');
createRoot(document.getElementById('root')).render(
  luWindow==='settings'?<SettingsWindow/>:
  luWindow==='model-library'?<ModelLibraryWindow/>:<App/>
);
