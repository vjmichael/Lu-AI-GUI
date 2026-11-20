import React,{useState,useEffect,useRef} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';

// The event only invalidates the view. Persisted native state is authoritative,
// and periodic refresh also covers lost events and app restart.
export function useScheduledReminders(){
  const [rows,setRows]=useState([]);
  const [error,setError]=useState('');
  const attempt=useRef(0);
  const live=useRef(false);
  async function refresh(){
    const current=++attempt.current;
    try{
      const result=await invoke('lu_scheduled_reminders');
      if(live.current&&current===attempt.current){setRows(result);setError('');}
    }catch(e){if(live.current&&current===attempt.current)setError('Could not refresh reminders. Try again.');}
  }
  useEffect(()=>{
    live.current=true;let stop=()=>{}, mounted=true;
    const interval=setInterval(()=>void refresh(),10000);
    void refresh();
    listen('lu-reminders-changed',()=>void refresh()).then(fn=>{if(mounted)stop=fn;else fn();}).catch(()=>{});
    return()=>{mounted=false;live.current=false;++attempt.current;clearInterval(interval);stop();};
  },[]);
  return {rows,error,refresh,unread:rows.reduce((n,r)=>n+r.unreadCount,0)};
}

const scheduleOptions=[[0,'Once'],[60,'Every hour'],[1440,'Every day (24 hours)'],[10080,'Every week (7 days)'],[1,'Every minute']];
export function ScheduledReminders({reminders,chats,chatId,onOpenChat}){
  const [owner,setOwner]=useState('');
  const [text,setText]=useState('');
  const [delay,setDelay]=useState('5');
  const [repeat,setRepeat]=useState('0');
  const [busy,setBusy]=useState(false);
  const [status,setStatus]=useState('');
  const [filter,setFilter]=useState('active');
  const [cancelId,setCancelId]=useState(null);
  const [limit,setLimit]=useState(25);
  const selected=chats.some(c=>c.chatId===owner)?owner:(chats.some(c=>c.chatId===chatId)?chatId:chats[0]?.chatId||'');
  const valid=selected&&text.trim()&&new TextEncoder().encode(text.trim()).length<=512&&Number.isInteger(Number(delay))&&Number(delay)>=1&&Number(delay)<=10080;
  async function create(e){
    e.preventDefault();if(!valid||busy)return;setBusy(true);setStatus('');
    try{
      const id=await invoke('lu_schedule_reminder',{chatId:selected,text:text.trim(),delayMinutes:Number(delay),repeatMinutes:Number(repeat)||null});
      if(id){setText('');setStatus('Reminder scheduled.');setFilter('active');await reminders.refresh();}
      else setStatus('Reminder not scheduled.');
    }catch(e){setStatus(String(e).includes('limit reached')?'Reminder limit reached.':'Could not schedule the reminder. Check your entry and try again.');}
    finally{setBusy(false);}
  }
  async function control(row,action){
    if(busy)return;setBusy(true);setStatus('');
    try{await invoke('lu_control_reminder',{chatId:row.workspaceId,reminderId:row.id,action,seenCount:action==='acknowledge'?row.firedCount:null});await reminders.refresh();setCancelId(null);}
    catch(e){setStatus('The reminder changed or could not be updated. Refresh and try again.');await reminders.refresh();}
    finally{setBusy(false);}
  }
  const visible=reminders.rows.filter(r=>filter==='all'||r.unreadCount||['ACTIVE','PAUSED'].includes(r.state));
  return <section className="scheduledReminders" aria-label="Scheduled reminders">
    <header><h2>Reminders</h2><p>Schedule a note for yourself or repeat it on a regular interval. You can also ask in Chat: “Remind me in 10 minutes to review my notes.”</p></header>
    <form onSubmit={create} className="reminderForm">
      <label>Conversation<select aria-label="Reminder conversation" value={selected} onChange={e=>setOwner(e.target.value)} disabled={busy||!chats.length}>
        {!chats.length&&<option value="">Create a conversation first</option>}{chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title||'New chat'}</option>)}
      </select></label>
      <label>Remind me to<input aria-label="Reminder text" type="text" maxLength={512} value={text} onChange={e=>setText(e.target.value)} placeholder="Review my notes" disabled={busy}/></label>
      <div className="reminderTiming"><label>In how many minutes?<input aria-label="Reminder delay in minutes" type="number" min="1" max="10080" step="1" value={delay} onChange={e=>setDelay(e.target.value)} disabled={busy}/></label>
        <label>Repeat<select aria-label="Reminder recurrence" value={repeat} onChange={e=>setRepeat(e.target.value)} disabled={busy}>{scheduleOptions.map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label></div>
      <small>Reminders appear inside Lu while it is open. Missed reminders appear when you reopen Lu. They do not run an AI or access files or the internet.</small>
      <button type="submit" className="primaryButton" disabled={!valid||busy}>Review reminder…</button>
    </form>
    {status&&<p role="status">{status}</p>}{reminders.error&&<p role="alert">{reminders.error}</p>}
    <div className="backgroundPaneToolbar"><div className="backgroundPaneFilters"><button type="button" className={filter==='active'?'selected':''} aria-pressed={filter==='active'} onClick={()=>{setFilter('active');setLimit(25);}}>Upcoming and due</button><button type="button" className={filter==='all'?'selected':''} aria-pressed={filter==='all'} onClick={()=>{setFilter('all');setLimit(25);}}>All reminders</button></div><button type="button" className="subtleButton" onClick={()=>void reminders.refresh()}>Refresh reminders</button></div>
    {visible.slice(0,limit).map(row=><article className="backgroundTask" aria-label={"Reminder: "+row.text} key={row.id}>
      <div className="backgroundTaskHeading"><strong>{row.text}</strong><span>{({ACTIVE:'Scheduled',PAUSED:'Paused',COMPLETED:'Completed',CANCELLED:'Cancelled'})[row.state]}</span></div>
      <small>{chats.find(c=>c.chatId===row.workspaceId)?.title||'Conversation'}</small>
      {row.unreadCount>0&&<p role="status"><strong>Reminder due{row.unreadCount>1?` (${row.unreadCount} occurrences)`:''}</strong> · {new Date(row.lastDue*1000).toLocaleString()}</p>}
      {['ACTIVE','PAUSED'].includes(row.state)&&<p>{row.state==='PAUSED'?'Paused':'Next reminder: '+new Date(row.nextDue*1000).toLocaleString()}{row.repeatSeconds?' · Every '+row.repeatSeconds/60+' minutes':''}</p>}
      <div className="nativeChoiceRow">
        {row.unreadCount>0&&<button type="button" disabled={busy} onClick={()=>void control(row,'acknowledge')}>Mark read</button>}
        {row.state==='ACTIVE'&&<button type="button" disabled={busy} onClick={()=>void control(row,'pause')}>Pause reminder</button>}
        {row.state==='PAUSED'&&<button type="button" disabled={busy} onClick={()=>void control(row,'resume')}>Resume reminder</button>}
        {['ACTIVE','PAUSED'].includes(row.state)&&<button type="button" disabled={busy} onClick={()=>setCancelId(row.id)}>Cancel reminder…</button>}
        <button type="button" className="subtleButton" onClick={()=>onOpenChat(row.workspaceId)}>Open conversation</button>
      </div>
      {cancelId===row.id&&<div className="nativeChoiceRow"><span>Stop future reminders?</span><button type="button" disabled={busy} onClick={()=>void control(row,'cancel')}>Confirm cancellation</button><button type="button" onClick={()=>setCancelId(null)}>Keep reminder</button></div>}
    </article>)}
    {visible.length>limit&&<button type="button" onClick={()=>setLimit(n=>n+25)}>Show more reminders</button>}
    {!visible.length&&!reminders.error&&<p>No reminders to show.</p>}
  </section>;
}
