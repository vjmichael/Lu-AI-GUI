import React,{useEffect,useMemo,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {NativeWindowFrame} from './native_windows.jsx';
import {FILTERS,taskKind,taskStatus,shownTasks,taskGroups,movedOrder} from './task_board_model.mjs';

const errorText=e=>String(e?.message||e||'Native task list unavailable');
const correction=t=>({
  summary:t.summary,description:t.description,priority:t.priority,
  blockedBy:t.blockedBy,acceptanceCriteria:t.acceptanceCriteria,reason:''
});
export function TaskBoardWindow(){
  const [boards,setBoards]=useState([]);
  const [registerId,setRegisterId]=useState('');
  const [filter,setFilter]=useState('All');
  const [search,setSearch]=useState('');
  const [taskId,setTaskId]=useState('');
  const [evidence,setEvidence]=useState(null);
  const [edit,setEdit]=useState(null);
  const [updatePreview,setUpdatePreview]=useState(null);
  const [modelSuggestion,setModelSuggestion]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  const current=boards.find(b=>b.board.registerId===(registerId||boards[0]?.board.registerId));
  const all=current?.tasks||[];
  const active=all.find(t=>t.taskId===taskId);
  const shown=useMemo(()=>shownTasks(all,filter,search),[all,filter,search]);
  const groups=useMemo(()=>taskGroups(shown),[shown]);
  async function reload(){
    const result=await invoke('lu_project_board_list');
    if(!Array.isArray(result))throw Error('Unrecognized native board response');
    setBoards(result);
    setRegisterId(previous=>result.some(b=>b.board.registerId===previous)
      ?previous:result[0]?.board.registerId||'');
    return result;
  }
  useEffect(()=>{let live=true;
    void invoke('lu_project_board_list').then(result=>{
      if(live){setBoards(result);setRegisterId(result[0]?.board.registerId||'');}
    }).catch(e=>{if(live)setError(errorText(e));});
    return ()=>{live=false};
  },[]);
  useEffect(()=>{setTaskId('');setEvidence(null);setEdit(null);setUpdatePreview(null);setModelSuggestion(null)},[registerId]);
  async function act(message,fn){
    if(busy)return;
    setBusy(true);setError('');setNotice('');
    try{
      const result=await fn();
      if(result!==null&&result!==false)setNotice(message);
      return result;
    }catch(e){setError(errorText(e));return null}
    finally{setBusy(false)}
  }
  async function selectTask(t){
    if(!current)return;
    setTaskId(t.taskId);setEvidence(null);setEdit(null);
    try{
      const detail=await invoke('lu_project_board_evidence',{
        registerId:current.board.registerId,taskId:t.taskId});
      setEvidence(detail);
    }catch(e){setError(errorText(e))}
  }
  async function reorder(task,delta){
    if(!current||filter!=='All'||search.trim())return;
    await act('Human ordering recorded as a draft in ANSF.',async()=>{
      const order=movedOrder(current.board.orderedTaskIds,task.taskId,delta);
      if(!order)return null;
      const saved=await invoke('lu_project_board_reorder',{
        registerId:current.board.registerId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256,
        orderedTaskIds:order
      });
      if(saved)await reload();
      return saved;
    });
  }
  async function saveCorrection(){
    if(!current||!active||!edit)return;
    await act('Native human correction added to original PLAN-05 task history.',async()=>{
      const saved=await invoke('lu_project_board_correct',{
        registerId:current.board.registerId,taskId:active.taskId,
        expectedVersion:active.version,correction:edit
      });
      if(saved){await reload();setEdit(null);setEvidence(null);}
      return saved;
    });
  }
  async function approve(){
    if(!current)return;
    await act('Plan wording and order reviewed, without any execution grant.',async()=>{
      const saved=await invoke('lu_project_board_approve',{
        registerId:current.board.registerId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256
      });
      if(saved)await reload();return saved;
    });
  }
  async function refreshEvidence(){
    if(!current)return;
    await act('Native source comparison recorded for human review; no task status was changed.',async()=>{
      const preview=await invoke('lu_project_update_inspect',{
        registerId:current.board.registerId
      });
      if(preview){setUpdatePreview(preview);setModelSuggestion(null);}
      return preview;
    });
  }
  async function reconcileEvidence(){
    if(!current||!updatePreview)return;
    await act('Source-change review appended; all new completion claims still require independent human verification.',async()=>{
      const result=await invoke('lu_project_update_apply',{
        previewSha256:updatePreview.digestSha256
      });
      if(result){setUpdatePreview(null);setModelSuggestion(null);await reload();setEvidence(null);}
      return result;
    });
  }
  async function suggestUpdatesWithGGUF(){
    if(!updatePreview)return;
    await act('Original local GGUF suggestions admitted as review-only evidence; no tasks were accepted.',async()=>{
      const result=await invoke('lu_project_update_model_suggest',{
        previewSha256:updatePreview.digestSha256
      });
      if(result)setModelSuggestion(result);
      return result;
    });
  }
  async function reloadOriginalModelReceipt(){
    if(!updatePreview)return;
    await act('Reopened and independently verified the original GGUF receipt from ANSF.',async()=>{
      const original=await invoke('lu_project_update_model_latest',{
        previewSha256:updatePreview.digestSha256
      });
      if(original)setModelSuggestion(original);
      return original;
    });
  }
  async function exportPlan(){
    if(!current)return;
    await act('Evidence-bound plan exported to a new operator-selected JSON file.',async()=>{
      return invoke('lu_project_board_export',{
        registerId:current.board.registerId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256
      });
    });
  }
  return <NativeWindowFrame title="Project Tasks"
    subtitle="Human-reviewed plans · original evidence · no model execution">
    <div className="plan06Layout" aria-label="Project task list">
      <header className="plan06Toolbar">
        <label>Project / task register
          <select aria-label="Project selector" disabled={busy}
            value={current?.board.registerId||''}
            onChange={e=>setRegisterId(e.target.value)}>
            {boards.map(b=><option key={b.board.registerId} value={b.board.registerId}>
              {b.projectScopeSha256.slice(0,12)} · {b.tasks.length} tasks · {b.board.registerId.slice(-9)}
            </option>)}
          </select>
        </label>
        <button type="button" disabled={busy}
          onClick={()=>void act('Audited ANSF registers refreshed.',reload)}>Refresh registers</button>
        {current&&<span role="status" className="plan06State">
          {current.reviewCurrent?'Human-reviewed plan · NOT executable':
            current.board.reviewState==='human_approved'?'Prior approval stale after task revisions':
            'Draft · no plan approval'}
        </span>}
      </header>
      {error&&<p role="alert" className="nativeError">{error}</p>}
      {notice&&<p role="status" className="nativeNotice">{notice}</p>}
      {!current?<p className="plan06Empty">No task registers yet. Use Settings to inspect a
        project, generate an evidence-backed proposal and import it after native human review.
        This window never creates tool authority.</p>:<>
        <div className="plan06Provenance">
          <small>Original proposal: {current.originalProposalSha256}</small>
          <small>Source register: {current.sourceRegisterSha256}</small>
          <small>Plan revision {current.board.sequence} · {current.eventCount} task-history events · {current.historicalApprovalCount} historical plan reviews</small>
        </div>
        <section className="plan07Refresh" aria-label="Evidence-driven task update review">
          <div className="plan07RefreshHeader">
            <strong>Evidence refresh · PLAN-07A</strong>
            <button type="button" disabled={busy}
              onClick={()=>void refreshEvidence()}>
              Recheck project source using native folder picker…
            </button>
          </div>
          <small>The host rereads the actual selected source and exact Git HEAD,
            compares audited evidence, and proposes review candidates only.
            Old acceptance remains historical. This is an evidence delta over
            complete bounded scans, not a filesystem watcher or an execution grant.</small>
          {updatePreview&&<details open>
            <summary>Current audited source comparison · {updatePreview.semanticChange?
              'source review needed':'no semantic source change'}</summary>
            <p>HEAD: {updatePreview.previousHead||'not verified'} → {updatePreview.currentHead||'not verified'}.
              Changed native evidence nodes: {updatePreview.changedEvidenceNodes}.
              Source scan: {updatePreview.scanComplete?'bounded complete':'incomplete'}.
              Exact criterion review candidates: {updatePreview.exactCriterionReviewCandidates}.</p>
            <small>Register lease: {updatePreview.expectedRegisterSha256}.
              Current graph SHA-256: {updatePreview.currentGraphSha256}.
              No task completion or model-derived source proof.</small>
            <div className="plan07Candidates">
              {updatePreview.candidates.map(candidate=><article key={candidate.taskId}>
                <strong>{candidate.taskId} · {candidate.disposition.replaceAll('_',' ')}</strong>
                <p>{candidate.explanation}</p>
                <small>Original requirement: {candidate.originalRequirementId} ·
                  Previous: {candidate.previousStatus} ·
                  Current: {candidate.currentStatus||'MISSING'} ·
                  Native health: {candidate.evidenceHealth||'insufficient'} ·
                  Task complete: NO</small>
              </article>)}
            </div>
            {updatePreview.changedListTruncated&&<p>Source-delta display is truncated;
              archived original evidence and task identities remain authoritative.</p>}
            <div className="plan07ModelSuggestions" aria-label="Local GGUF evidence review suggestions">
              <h3>Local GGUF suggestions · PLAN-07B</h3>
              <p>Optional original-model suggestions only. Native citations and status
                are checked by the host; no result completes or executes a task.</p>
              <div className="plan06Actions">
                <button type="button" disabled={busy} onClick={()=>void suggestUpdatesWithGGUF()}>
                  Ask loaded local GGUF for evidence suggestions…
                </button>
                <button type="button" disabled={busy}
                  onClick={()=>void reloadOriginalModelReceipt()}>
                  Reopen audited original GGUF receipt…
                </button>
              </div>
              {modelSuggestion?.previewSha256===updatePreview.digestSha256&&
                <section aria-label="Original model suggestions, not accepted work">
                  <small>Model: {modelSuggestion.originalModelRef}.
                    Session: {modelSuggestion.originalSessionId}.
                    Original reply SHA-256: {modelSuggestion.originalReplySha256}.
                    Source register SHA-256: {modelSuggestion.expectedRegisterSha256}.
                  </small>
                  {modelSuggestion.suggestions.map(t=><article key={t.taskId}>
                    <strong>{t.taskId} · {t.action.replaceAll('_',' ')}
                      · SUGGESTED, NOT ACCEPTED</strong>
                    <p>{t.reason}</p>
                    <p><strong>Future independent evidence:</strong> {t.nextEvidence}</p>
                    <small>Original requirement: {t.originalRequirementId}.
                      Host status: {t.hostStatus||'missing'}.
                      Citations: {t.evidenceIds.join(', ')}.
                      Task completed: NO. Execution authorized: NO.
                    </small>
                  </article>)}
                </section>}
            </div>
            <button type="button" disabled={busy||!updatePreview.semanticChange}
              onClick={()=>void reconcileEvidence()}>
              Review native source changes and append revisions…
            </button>
          </details>}
        </section>
        <div className="plan06Columns">
          <section className="plan06Tasks" aria-label="Categorized project tasks">
            <div className="plan06Search">
              <label>Find a task
                <input aria-label="Search project tasks" value={search}
                  placeholder="Task, source, requirement…"
                  onChange={e=>setSearch(e.target.value)}/></label>
              <nav className="plan06Filters" aria-label="Task status filters">
                {FILTERS.map(label=><button type="button" key={label}
                  aria-pressed={filter===label} className={filter===label?'selected':''}
                  disabled={busy} onClick={()=>setFilter(label)}>
                    {label} ({label==='All'?all.length:all.filter(t=>taskKind(t)===label).length})
                  </button>)}
              </nav>
              <small>“Completed” filters only independently verified exact requirements;
                a full task is not proven completed. Clear filters to reorder.</small>
            </div>
            <div className="plan06Groups">
              {groups.map(group=><section key={group.name} aria-label={group.name}>
                <h2>{group.name}</h2>
                {group.tasks.map(task=>{
                  const i=all.findIndex(t=>t.taskId===task.taskId);
                  return <article key={task.taskId} className="plan06Task">
                    <button type="button" className={task.taskId===taskId?'selected':''}
                      aria-current={task.taskId===taskId?'true':undefined}
                      onClick={()=>void selectTask(task)}>
                      <strong>{task.priority} · {task.summary}</strong>
                      <span>{taskStatus(task)}</span>
                      <small>v{task.version} · {task.sourceRequirementId.slice(0,16)}</small>
                    </button>
                    <div className="plan06Order" aria-label={'Reorder '+task.summary}>
                      <button type="button" disabled={busy||filter!=='All'||Boolean(search.trim())||i<=0}
                        aria-label={'Move '+task.summary+' up'} onClick={()=>void reorder(task,-1)}>↑</button>
                      <button type="button" disabled={busy||filter!=='All'||Boolean(search.trim())||i===all.length-1}
                        aria-label={'Move '+task.summary+' down'} onClick={()=>void reorder(task,1)}>↓</button>
                    </div>
                  </article>;
                })}
              </section>)}
              {!shown.length&&<p>No tasks match this filter. No completion is inferred.</p>}
            </div>
          </section>
          <aside className="plan06Details" aria-label="Selected task and original evidence">
            {!active?<p>Select a task to inspect its original source, verification status,
              evidence relationships, and human revision history.</p>:<>
              <h2>{active.summary}</h2>
              <small>{active.taskId} · v{active.version}</small>
              <p>{active.description}</p>
              <p><strong>Classification:</strong> {taskStatus(active)}</p>
              <p><strong>Acceptance criterion:</strong> {active.acceptanceCriteria}</p>
              <p><strong>Blocker:</strong> {active.blockedBy}</p>
              <p><strong>Exact HEAD:</strong> {active.selectedHead||'unverified'}</p>
              <p><strong>Original requirement:</strong> {active.sourceRequirementId}</p>
              <button type="button" disabled={busy}
                onClick={()=>setEdit(correction(active))}>Edit task wording…</button>
              {edit&&<section className="plan06Editor" aria-label="Human task edit form">
                <label>Summary<input maxLength={100} value={edit.summary}
                  onChange={e=>setEdit(p=>({...p,summary:e.target.value}))}/></label>
                <label>Description<textarea rows={4} maxLength={300} value={edit.description}
                  onChange={e=>setEdit(p=>({...p,description:e.target.value}))}/></label>
                <label>Priority<select value={edit.priority}
                  onChange={e=>setEdit(p=>({...p,priority:e.target.value}))}>
                  {['P0','P1','P2'].map(p=><option key={p}>{p}</option>)}
                </select></label>
                <label>Blocker<input maxLength={200} value={edit.blockedBy}
                  onChange={e=>setEdit(p=>({...p,blockedBy:e.target.value}))}/></label>
                <label>Acceptance criterion<textarea rows={3} maxLength={300}
                  value={edit.acceptanceCriteria}
                  onChange={e=>setEdit(p=>({...p,acceptanceCriteria:e.target.value}))}/></label>
                <label>Reason for correction<input maxLength={250} value={edit.reason}
                  onChange={e=>setEdit(p=>({...p,reason:e.target.value}))}/></label>
                <div className="plan06Actions">
                  <button type="button" disabled={busy||edit.reason.trim().length<5}
                    onClick={()=>void saveCorrection()}>Review correction in OS dialog…</button>
                  <button type="button" disabled={busy}
                    onClick={()=>setEdit(null)}>Cancel edit</button>
                </div>
                <small>Source requirement IDs remain immutable. Editing invalidates
                  prior plan approval, not original acceptance history.</small>
              </section>}
              {evidence?.taskId===taskId?<section className="plan06Sources" aria-label="Evidence details">
                <h3>Native source evidence</h3>
                <p>{evidence.finding?.primary||'UNKNOWN'} · {evidence.finding?.confidence||'Insufficient'}
                  · {evidence.finding?.evidenceHealth||'unverified'}</p>
                <p>{evidence.finding?.explanation||'No independently established result.'}</p>
                <small>Graph: {evidence.sourceGraphSha256} · Status: {evidence.sourceStatusSha256}</small>
                <details open><summary>Source nodes ({evidence.nodes.length})</summary>
                  {evidence.nodes.map(node=><article key={node.id}>
                    <strong>{node.kind} · {node.evidence}</strong>
                    <p>{node.label}</p><small>{node.locator}{node.line?':'+node.line:''}</small>
                    <small>{node.digest||'No source byte digest'}</small>
                  </article>)}
                </details>
                <details><summary>Evidence edges ({evidence.edges.length})</summary>
                  {evidence.edges.map((edge,i)=><p key={i}>
                    {edge.relation} · {edge.evidence} · {edge.reason}
                  </p>)}
                </details>
                <details><summary>Original revisions ({evidence.history.length})</summary>
                  {evidence.history.map((event,i)=><p key={i}>
                    #{event.sequence} · v{event.version} · {event.kind} · {event.reason}
                  </p>)}
                </details>
              </section>:<p>Original evidence loading or not yet opened.</p>}
            </>}
          </aside>
        </div>
        <footer className="plan06Footer">
          <div className="plan06Actions">
            <button type="button" disabled={busy||current.reviewCurrent}
              onClick={()=>void approve()}>Approve plan wording and order only…</button>
            <button type="button" disabled={busy}
              onClick={()=>void exportPlan()}>Export evidence-bound JSON plan…</button>
          </div>
          <small>Review never grants file writes, Git operations, worker scheduling,
            native tools or task completion. Those require separate authorization and evidence.</small>
        </footer>
      </>}
    </div>
  </NativeWindowFrame>;
}
