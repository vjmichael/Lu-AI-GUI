import React,{useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';

export const LU_TABS=[['chat','Chat'],['history','History'],['tasks','Tasks'],['models','Models'],['settings','Settings']];
export function friendlyError(error){
  const message=String(error?.message||error||'');
  if(/CONTEXT_LIMIT/i.test(message))return 'This request is too large for the current model context. Try a smaller image or increase conversation context in Settings.';
  if(/VISION_UNAVAILABLE|compatible MMProj|no admitted MMProj|projector before image/i.test(message))return 'Load a model with its matching vision companion to read images.';
  if(/MEDIA_REJECTED/i.test(message))return 'The model could not read this image. Try a different image or format.';
  if(/TEMPLATE_REJECTED/i.test(message))return 'This model could not process the request. Reload the model and try again.';
  if(/HTTP 400|image generation|image request/i.test(message))return 'The model rejected the request. Reload the model and try again. Your unsent image is kept if the draft is unchanged.';
  if(/model session changed|Active inference session changed|No loaded model/i.test(message))return 'The model is no longer loaded. Choose a model and try again.';
  if(/Observe & Learn is active|unscoped network/i.test(message))return 'Internet access is paused while Observe & Learn is active. Stop observation in Settings before searching online.';
  if(/catalog search unavailable|catalog returned|catalog search response/i.test(message))return 'Could not reach the model catalog. Check your connection and try again.';
  if(/InferenceFailed|Bridge\(|Engine\(|Backend\(/i.test(message))return 'The model could not complete this request. Try again or reload the model.';
  if(/unsupported attachment/i.test(message))return 'This file type is not supported in chat yet.';
  if(/cancel|declin/i.test(message))return 'The operation was cancelled or declined.';
  return 'Lu could not complete this operation. Please try again.';
}
export function formatTaskState(state){
  return ({PENDING:'Waiting',QUEUED:'Queued',RUNNING:'Running',PAUSED:'Paused',COMPLETED:'Completed',SUCCEEDED:'Completed',CANCELLED:'Cancelled',FAILED:'Failed',INTERRUPTED:'Interrupted',NEEDS_REVIEW:'Needs approval',TIMED_OUT:'Timed out',REVIEWED:'Preparing',VERIFIED_AT_PUBLICATION:'Downloaded',INTERRUPTED_REVIEW_REQUIRED:'Interrupted'})[state]||'Status unavailable';
}
export function taskName(task){
  const detail=String(task.detail||'').toLowerCase();
  if(/capture|screen|frame/.test(detail))return 'Screen capture';
  if(/scan|checksum|sha-256|hash/.test(detail))return 'File scan';
  if(/browser|chromium|web page/.test(detail))return 'Web browsing';
  if(/research|document/.test(detail))return 'Research';
  return 'Background task';
}
export function HistoryTab({chats,onOpenChat}){
  const [query,setQuery]=useState('');
  const [searched,setSearched]=useState('');
  const [results,setResults]=useState([]);
  const [next,setNext]=useState(null);
  const [reading,setReading]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const sequence=useRef(0);
  const titles=chats.filter(c=>!searched||c.title.toLowerCase().includes(searched.toLowerCase()));
  async function search(event,more=false){
    event?.preventDefault();
    const q=more?searched:query.trim();
    if(!q||busy)return;
    const id=++sequence.current;
    setBusy(true);setError('');setReading(null);
    if(!more){setSearched(q);setResults([]);setNext(null);}
    try{
      const page=await invoke('lu_history_search',{query:q,offset:more?next:0});
      if(id!==sequence.current)return;
      setResults(old=>[...new Map((more?[...old,...page.hits]:page.hits).map(hit=>[hit.sourceId,hit])).values()]);
      setNext(page.nextOffset);
    }catch(e){if(id===sequence.current)setError(friendlyError(e));}
    finally{if(id===sequence.current)setBusy(false);}
  }
  async function read(hit,more=false){
    if(busy)return;
    const id=++sequence.current;
    setBusy(true);setError('');
    try{
      const page=await invoke('lu_m3_source_text_page',{chatId:hit.sourceChatId,sourceId:hit.sourceId,
        offsetChars:more?reading.nextOffsetChars:0,limitChars:4096});
      if(id!==sequence.current)return;
      setReading(old=>({...page,chatTitle:hit.chatTitle,text:more&&old?.sourceId===page.sourceId?old.text+page.text:page.text}));
    }catch(e){if(id===sequence.current)setError(friendlyError(e));}
    finally{if(id===sequence.current)setBusy(false);}
  }
  return <div className="simpleHistory" aria-label="Conversation history">
    <header><h1>History</h1><p>Search your saved conversations and messages.</p></header>
    <form className="simpleHistorySearch" onSubmit={search}>
      <input aria-label="Search all conversations" type="search" value={query}
        maxLength={200} placeholder="Search all conversations…" onChange={e=>setQuery(e.target.value)}/>
      <button className="primaryButton" type="submit" disabled={busy||!query.trim()}>Search</button>
      {searched&&<button type="button" className="subtleButton" disabled={busy} onClick={()=>{sequence.current++;setSearched('');setQuery('');setResults([]);setNext(null);setReading(null);setError('');}}>Clear</button>}
    </form>
    {error&&<p role="alert" className="nativeError">{error}</p>}
    {busy&&<p role="status">Searching or reading history…</p>}
    <div className="simpleHistoryResults">
      <h2>{searched?'Matching conversations':'Recent conversations'}</h2>
      {titles.map(chat=><button type="button" className="historyResult" key={chat.chatId} onClick={()=>onOpenChat(chat.chatId)}>
        <strong>{chat.title||'New chat'}</strong><span>Open conversation →</span>
      </button>)}
      {!searched&&!chats.length&&<p>No saved conversations yet.</p>}
      {searched&&<><h2>Matching messages</h2>
        {results.map(hit=><article className="historyResult" key={hit.sourceId}>
          <strong>{hit.chatTitle}</strong><p>{hit.excerpt}{hit.truncated?'…':''}</p>
          <small>{hit.role==='user'?'You':'Lu'}</small>
          <div className="nativeChoiceRow"><button className="subtleButton" disabled={busy} onClick={()=>void read(hit)}>Read message</button>
            <button className="subtleButton" onClick={()=>onOpenChat(hit.sourceChatId)}>Open conversation</button></div>
        </article>)}
        {!busy&&next===null&&!results.length&&<p>No matching messages found.</p>}
        {next!==null&&<button type="button" className="subtleButton" disabled={busy} onClick={()=>void search(null,true)}>Search more conversations</button>}
      </>}
    </div>
    {reading&&<section className="historyReading" aria-label="Saved message">
      <header><div><strong>{reading.chatTitle}</strong><p>{reading.role==='user'?'You':'Lu'}</p></div>
        <button type="button" className="subtleButton" onClick={()=>setReading(null)}>Close message</button></header>
      <p className="historyOriginal">{reading.text}</p>
      {reading.nextOffsetChars!==null&&<button className="subtleButton" disabled={busy} onClick={()=>void read(reading,true)}>Read more</button>}
      <button type="button" className="subtleButton" onClick={()=>onOpenChat(reading.sourceChatId)}>Open conversation</button>
    </section>}
  </div>;
}
