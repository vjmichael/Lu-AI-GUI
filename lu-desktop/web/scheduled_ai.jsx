import React,{useState,useEffect,useRef} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
const schedules=[[0,'Once'],[1,'Every minute'],[60,'Every hour'],[1440,'Every day (24 hours)'],[10080,'Every week']];
const states={ACTIVE:'Scheduled',PAUSED:'Paused',COMPLETED:'Finished',CANCELLED:'Cancelled',RUNNING:'Working',FAILED:'Could not finish',STOPPED:'Stopped',INTERRUPTED:'Interrupted'};
export function ScheduledAI({chats,chatId,runtime,models,onOpenChat}){
  const [rows,setRows]=useState([]),[owner,setOwner]=useState(''),[prompt,setPrompt]=useState(''),[delay,setDelay]=useState('5'),[repeat,setRepeat]=useState('0'),[busy,setBusy]=useState(false),[message,setMessage]=useState(''),[error,setError]=useState(''),[expanded,setExpanded]=useState(null),[resultView,setResultView]=useState({id:null,runs:[]}),[cancel,setCancel]=useState(null);
  const current=useRef(0),currentRuns=useRef(0),live=useRef(false);
  const selected=chats.some(c=>c.chatId===owner)?owner:chats.some(c=>c.chatId===chatId)?chatId:chats[0]?.chatId||'';
  const model=models.find(m=>m.modelRef===runtime.modelRef);
  const ready=runtime.phase==='Ready'&&!!model;
  const valid=ready&&selected&&prompt.trim()&&new TextEncoder().encode(prompt.trim()).length<=512&&Number.isInteger(Number(delay))&&Number(delay)>=1&&Number(delay)<=10080;
  async function refresh(){const n=++current.current;try{const r=await invoke('lu_ai_schedules');if(live.current&&current.current===n){setRows(r);setError('');}}catch{if(live.current&&current.current===n)setError('Could not refresh scheduled tasks.');}}
  useEffect(()=>{live.current=true;let stop=()=>{},mounted=true;void refresh();const timer=setInterval(()=>void refresh(),2000);listen('lu-scheduled-ai-changed',()=>void refresh()).then(fn=>{if(mounted)stop=fn;else fn();}).catch(()=>{});return()=>{live.current=false;mounted=false;++current.current;clearInterval(timer);stop();};},[]);
  const openRow=rows.find(r=>r.id===expanded);
  const runs=resultView.id===openRow?.id?resultView.runs:[];
  useEffect(()=>{setResultView({id:null,runs:[]});if(!openRow)return;let mounted=true;async function refreshRuns(){const n=++currentRuns.current;try{const r=await invoke('lu_ai_schedule_runs',{chatId:openRow.workspaceId,scheduleId:openRow.id});if(mounted&&currentRuns.current===n)setResultView({id:openRow.id,runs:r});}catch{if(mounted&&currentRuns.current===n)setMessage('Could not read task results.');}}void refreshRuns();const timer=setInterval(()=>void refreshRuns(),1500);return()=>{mounted=false;++currentRuns.current;clearInterval(timer);};},[openRow?.id,openRow?.workspaceId]);
  async function create(e){e.preventDefault();if(!valid||busy)return;setBusy(true);setMessage('');try{const id=await invoke('lu_schedule_ai',{chatId:selected,prompt:prompt.trim(),delayMinutes:Number(delay),repeatMinutes:Number(repeat)||null});setMessage(id?'Task scheduled.':'Task not scheduled.');if(id){setPrompt('');await refresh();setExpanded(id);}}catch{setMessage('Could not schedule the task. Check that the model is ready and try again.');}finally{setBusy(false);}}
  async function control(row,action){if(busy)return;setBusy(true);setMessage('');try{await invoke('lu_control_ai_schedule',{chatId:row.workspaceId,scheduleId:row.id,action});setCancel(null);await refresh();}catch{setMessage('The task could not be updated. If it is working, cancel it or wait for the result.');}finally{setBusy(false);}}
  return <section className="scheduledReminders" aria-label="Scheduled AI tasks">
    <header><h2>Scheduled AI tasks</h2><p>Ask your loaded model to write a short response at a later time or regular interval.</p></header>
    <form className="reminderForm" onSubmit={create}>
      <label>Conversation<select aria-label="Task conversation" value={selected} disabled={busy||!chats.length} onChange={e=>setOwner(e.target.value)}>{!chats.length&&<option value="">Create a conversation first</option>}{chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title||'New chat'}</option>)}</select></label>
      <label>Ask Lu to<input aria-label="Scheduled task prompt" value={prompt} maxLength={512} placeholder="Write a short daily reflection" disabled={busy} onChange={e=>setPrompt(e.target.value)}/></label>
      <div className="reminderTiming"><label>In how many minutes?<input aria-label="Task delay in minutes" type="number" min="1" max="10080" value={delay} disabled={busy} onChange={e=>setDelay(e.target.value)}/></label><label>Repeat<select aria-label="Task recurrence" disabled={busy} value={repeat} onChange={e=>setRepeat(e.target.value)}>{schedules.map(([n,label])=><option key={n} value={n}>{label}</option>)}</select></label></div>
      <p>{ready?'Model: '+model.filename:'Load a model and wait until it is ready to schedule a task.'}</p>
      <small>Runs only while Lu is open and the approved model is loaded and idle. No files, internet or conversation history. Up to 32 attempts per task; missed repeats are combined. Chats have priority.</small>
      <button type="submit" className="primaryButton" disabled={!valid||busy}>Review task…</button>
    </form>
    {message&&<p role="status">{message}</p>}{error&&<p role="alert">{error}</p>}
    <button type="button" className="subtleButton" onClick={()=>void refresh()}>Refresh scheduled tasks</button>
    {rows.map(row=><article key={row.id} className="backgroundTask" aria-label={'Scheduled task: '+row.prompt}>
      <div className="backgroundTaskHeading"><strong>{row.prompt}</strong><span>{row.running?'Working':states[row.state]||'Unavailable'}</span></div>
      <small>{chats.find(c=>c.chatId===row.workspaceId)?.title||'Conversation'} · {models.find(m=>m.modelRef===row.modelRef)?.filename||'Approved model unavailable'}</small>
      <p>{row.runCount} of 32 attempts{row.state==='ACTIVE'&&!row.running?' · '+(row.nextDue*1000<=Date.now()?'Due — waiting for an available model':'Next: '+new Date(row.nextDue*1000).toLocaleString()):''}</p>
      <div className="nativeChoiceRow">{row.state==='ACTIVE'&&!row.running&&<button type="button" disabled={busy} onClick={()=>void control(row,'pause')}>Pause task</button>}{row.state==='PAUSED'&&<button type="button" disabled={busy} onClick={()=>void control(row,'resume')}>Resume task</button>}{['ACTIVE','PAUSED'].includes(row.state)&&<button type="button" disabled={busy} onClick={()=>setCancel(row.id)}>Cancel task…</button>}<button type="button" aria-expanded={expanded===row.id} onClick={()=>setExpanded(expanded===row.id?null:row.id)}>{expanded===row.id?'Hide results':'View results'}</button><button type="button" className="subtleButton" onClick={()=>onOpenChat(row.workspaceId)}>Open conversation</button></div>
      {cancel===row.id&&<div className="nativeChoiceRow"><span>Stop this task and all future runs? The model may unload.</span><button type="button" disabled={busy} onClick={()=>void control(row,'cancel')}>Confirm task cancellation</button><button type="button" onClick={()=>setCancel(null)}>Keep task</button></div>}
      {expanded===row.id&&<section aria-label="Task results">{runs.map(run=><div key={run.sequence} className="backgroundTask"><strong>Attempt {run.sequence} — {states[run.state]||'Unavailable'}</strong><small>{new Date(run.started*1000).toLocaleString()}</small>{run.state==='COMPLETED'&&<p style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{run.output}</p>}</div>)}{!runs.length&&<p>No results yet.</p>}</section>}
    </article>)}
    {!rows.length&&!error&&<p>No scheduled AI tasks.</p>}
  </section>;
}
