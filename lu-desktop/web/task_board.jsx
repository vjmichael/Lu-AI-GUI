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
  const [reviewHistory,setReviewHistory]=useState([]);
  const [agentMode,setAgentMode]=useState('review_only');
  const [agentLinks,setAgentLinks]=useState([]);
  const [criterionReviews,setCriterionReviews]=useState([]);
  const [criterionReason,setCriterionReason]=useState('');
  const [reviewTaskId,setReviewTaskId]=useState('');
  const [reviewMode,setReviewMode]=useState('native');
  const [reviewNote,setReviewNote]=useState('');
  const [modelBusy,setModelBusy]=useState(false);
  const [modelCancelBusy,setModelCancelBusy]=useState(false);
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
  useEffect(()=>{
    setTaskId('');setEvidence(null);setEdit(null);setUpdatePreview(null);
    setModelSuggestion(null);setReviewTaskId('');setReviewMode('native');
    setReviewNote('');setReviewHistory([]);setAgentLinks([]);
    setCriterionReviews([]);setCriterionReason('');
    if(!registerId)return;
    let live=true;
    void invoke('lu_project_update_review_history',{registerId})
      .then(rows=>{if(live)setReviewHistory(Array.isArray(rows)?rows:[])})
      .catch(e=>{if(live)setError(errorText(e))});
    return ()=>{live=false};
  },[registerId]);
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
    setAgentLinks([]);setCriterionReviews([]);setCriterionReason('');
    try{
      const [detail,links,reviews]=await Promise.all([
        invoke('lu_project_board_evidence',{
          registerId:current.board.registerId,taskId:t.taskId}),
        invoke('lu_project_task_agent_handoffs',{
          registerId:current.board.registerId,taskId:t.taskId}),
        invoke('lu_project_task_criterion_evidence_history',{
          registerId:current.board.registerId,taskId:t.taskId})
      ]);
      setEvidence(detail);setAgentLinks(links);setCriterionReviews(reviews);
    }catch(e){setError(errorText(e))}
  }
  async function reopenAgentLinks(){
    if(!current||!active)return;
    await act('Reopened original task-to-agent plan links; no task completed.',async()=>{
      const links=await invoke('lu_project_task_agent_handoffs',{
        registerId:current.board.registerId,taskId:active.taskId
      });
      setAgentLinks(links);return links;
    });
  }
  async function reopenCriterionReviews(){
    if(!current||!active||busy)return;
    await act('Original human criterion-evidence decisions reopened, not task acceptance.',async()=>{
      const reviews=await invoke('lu_project_task_criterion_evidence_history',{
        registerId:current.board.registerId,taskId:active.taskId
      });
      setCriterionReviews(reviews);return reviews;
    });
  }
  async function reviewIndependentWitness(link,result,decision){
    if(!current||!active||busy||!result.reviewEligible||
       (decision==='ACCEPTED_AS_SUPPORTING_EVIDENCE'&&
        !result.criterionMethodReviewable)||
       criterionReason.trim().length<10||
       !current.reviewCurrent||active.needsHumanReconciliation)return;
    await act('Human decision appended to original ANSF criterion-evidence journal. Task status and tool authority unchanged.',async()=>{
      const entry=await invoke('lu_project_task_review_independent_witness',{
        registerId:current.board.registerId,taskId:active.taskId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256,
        expectedTaskVersion:active.version,
        handoffId:link.handoffId,receiptSha256:result.receiptSha256,
        decision,operatorReason:criterionReason.trim()
      });
      if(entry){
        const [reviews,links]=await Promise.all([
          invoke('lu_project_task_criterion_evidence_history',{
            registerId:current.board.registerId,taskId:active.taskId
          }),
          invoke('lu_project_task_agent_handoffs',{
            registerId:current.board.registerId,taskId:active.taskId
          })
        ]);
        setCriterionReviews(reviews);setAgentLinks(links);
        setCriterionReason('');
      }
      return entry;
    });
  }
  async function prepareAgentPlan(){
    if(!current||!active||busy||!current.reviewCurrent||
       active.needsHumanReconciliation)return;
    await act('Created a separately reviewed source-linked agent draft. Open its dedicated conversation in the main Lu window. No worker executed.',async()=>{
      const link=await invoke('lu_project_task_prepare_agent_plan',{
        registerId:current.board.registerId,taskId:active.taskId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256,
        expectedTaskVersion:active.version,mode:agentMode
      });
      if(link){
        const links=await invoke('lu_project_task_agent_handoffs',{
          registerId:current.board.registerId,taskId:active.taskId
        });
        setAgentLinks(links);
      }
      return link;
    });
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
      if(preview){setUpdatePreview(preview);setModelSuggestion(null);
        setReviewTaskId(preview.candidates[0]?.taskId||'');
        setReviewMode('native');setReviewNote('');
      }
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
    if(!updatePreview||busy)return;
    setModelBusy(true);
    try{
      await act('Original local GGUF suggestions admitted as review-only evidence; no tasks were accepted.',async()=>{
        const result=await invoke('lu_project_update_model_suggest',{
          previewSha256:updatePreview.digestSha256
        });
        if(result)setModelSuggestion(result);
        return result;
      });
    }finally{setModelBusy(false)}
  }
  async function cancelGgufSuggestions(){
    if(!modelBusy||modelCancelBusy)return;
    setModelCancelBusy(true);
    try{
      const canceled=await invoke('lu_project_update_model_cancel');
      setNotice(canceled
        ?'Native cancellation requested; no model suggestion will be accepted from the interrupted generation.'
        :'No PLAN-07B generation currently queued or active. A pending OS dialog cannot be canceled by this control.');
    }catch(e){setError(errorText(e))}
    finally{setModelCancelBusy(false)}
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
  async function reviewSuggestion(){
    if(!current||!updatePreview||reviewNote.trim().length<5||busy)return;
    const task=reviewTaskId||updatePreview.candidates[0]?.taskId;
    if(!task)return;
    const original=modelSuggestion?.suggestions?.find(s=>s.taskId===task);
    if(reviewMode==='model'&&(!original||
      modelSuggestion.previewSha256!==updatePreview.digestSha256)){
      setError('Reopen the original matching GGUF suggestion before reviewing it.');
      return;
    }
    await act('Native human evidence-review decision recorded; no task completion or execution authorized.',async()=>{
      const decision=await invoke('lu_project_update_review_decide',{
        previewSha256:updatePreview.digestSha256,taskId:task,
        modelSuggestionSha256:reviewMode==='model'?modelSuggestion.digestSha256:null,
        operatorNote:reviewNote.trim()
      });
      if(decision){
        const rows=await invoke('lu_project_update_review_history',{
          registerId:current.board.registerId
        });
        setReviewHistory(rows);
        setReviewNote('');
        setNotice('Recorded '+decision.phase.replaceAll('_',' ')+' for '+decision.taskId+
          '. The task register and source requirements remain unchanged.');
      }
      return decision;
    });
  }
  async function revalidateExactOriginalRequirement(){
    if(!current||busy)return;
    const selected=reviewTaskId||updatePreview?.candidates[0]?.taskId;
    const revision=all.find(t=>t.taskId===selected);
    if(!revision){
      setError('Choose a current original task before independent verification.');
      return;
    }
    await act('Exact original native criterion independently revalidated by a person; no whole-task completion or tools authorized.',async()=>{
      const result=await invoke('lu_project_update_review_exact_requirement',{
        registerId:current.board.registerId,
        taskId:revision.taskId,expectedVersion:revision.version
      });
      if(result){
        setUpdatePreview(null);setModelSuggestion(null);setReviewNote('');
        await reload();setEvidence(null);
      }
      return result;
    });
  }
  async function refreshReviewHistory(){
    if(!current)return;
    await act('Original approval/refusal/restart history independently audited.',async()=>{
      const rows=await invoke('lu_project_update_review_history',{
        registerId:current.board.registerId
      });
      setReviewHistory(rows);
      return rows;
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
                <button type="button" disabled={!modelBusy||modelCancelBusy}
                  onClick={()=>void cancelGgufSuggestions()}>
                  {modelCancelBusy?'Canceling local GGUF…':'Cancel queued/active GGUF generation…'}
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
        <section className="plan07HumanReview" aria-label="Human evidence-review decision journal">
          <div className="plan07HumanHeader">
            <h3>Human evidence decisions · PLAN-07C</h3>
            <button type="button" disabled={busy||!current} onClick={()=>void refreshReviewHistory()}>
              Reopen original decision and restart history…
            </button>
          </div>
          <p>Approve or refuse the NEXT EVIDENCE REVIEW only. Approval cannot mark a
            requirement accepted, complete the engineering task, or run tools.
            Changes to original task wording remain human-owned through the
            separate task correction form. Native source reconciliation is
            separately authorized above.</p>
          {updatePreview&&<>
            <label>Original task to review
              <select aria-label="Original task for human evidence review"
                value={reviewTaskId||updatePreview.candidates[0]?.taskId||''}
                onChange={e=>setReviewTaskId(e.target.value)}>
                {updatePreview.candidates.map(t=><option key={t.taskId} value={t.taskId}>
                  {t.taskId} · {t.disposition.replaceAll('_',' ')}
                </option>)}
              </select>
            </label>
            <fieldset>
              <legend>What are you reviewing?</legend>
              <label><input type="radio" name="plan07c-review-origin" value="native"
                checked={reviewMode==='native'}
                onChange={()=>setReviewMode('native')}/> Native deterministic evidence</label>
              <label><input type="radio" name="plan07c-review-origin" value="model"
                checked={reviewMode==='model'}
                disabled={!modelSuggestion?.suggestions?.some(t=>
                  t.taskId===(reviewTaskId||updatePreview.candidates[0]?.taskId))}
                onChange={()=>setReviewMode('model')}/> Original GGUF suggestion (untrusted)</label>
            </fieldset>
            <label>Your independent review note (5–250 characters)
              <textarea aria-label="Reason for human evidence decision"
                value={reviewNote} maxLength={250} rows={3}
                onChange={e=>setReviewNote(e.target.value)}
                placeholder="What independent evidence should be examined or why refuse?"/>
            </label>
            <button type="button" disabled={busy||reviewNote.trim().length<5}
              onClick={()=>void reviewSuggestion()}>
              Open native OS approve/refuse decision…
            </button>
            <div className="plan07ExactRevalidation">
              <button type="button" disabled={busy||!current}
                onClick={()=>void revalidateExactOriginalRequirement()}>
                Independently review EXACT native requirement (separate approval)…
              </button>
              <small>This second OS approval is distinct from accepting a model
                suggestion. The host compares the current original source with
                the latest accepted register snapshot, repeats the scan after
                approval and writes an exact-criterion-only human revision.
                Changed or unreconciled source is refused until the separate
                source reconciliation above has been completed.</small>
            </div>
            <small>The host commits a pending receipt before the OS dialog.
              Approval requires selecting the same original project again and
              a fresh source scan; refusal, cancellation, stale evidence, and
              interrupted reviews remain durably recorded.</small>
          </>}
          <div className="plan07ReviewHistory" role="region" aria-label="Original human review history">
            {reviewHistory.length===0?<small>No human PLAN-07C review decisions yet.</small>:
              reviewHistory.slice(-24).reverse().map(e=><article key={e.reviewId}>
                <strong>{e.taskId} v{e.expectedTaskVersion} · {e.phase.replaceAll('_',' ')}</strong>
                <p>{e.operatorNote}</p>
                <small>Original source: {e.originalRequirementId} ·
                  Review SHA: {e.sha256} · Register lease: {e.expectedRegisterSha256} ·
                  Model suggestion: {e.modelSuggestionSha256||'none'} ·
                  Completed work: NO · Execution authority: NO</small>
              </article>)}
          </div>
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
              <section className="plan06AgentHandoff" aria-label="Selected task agent planning handoff">
                <h3>Agent plan from selected task · P0-Next</h3>
                <p>This creates a separate ANSF conversation and human-review-only agent plan
                  from the original task and a freshly selected native project source.
                  Task-board approval is not tool approval; the task is never marked complete.</p>
                <label>Agent draft mode
                  <select aria-label="Agent task plan mode" value={agentMode}
                    disabled={busy} onChange={e=>setAgentMode(e.target.value)}>
                    <option value="review_only">Review-only source, analysis and human decision (no installed worker adapter)</option>
                    <option value="reviewed_git_media_check">One independently reviewed Git/media source check (separate native tool approval required)</option>
                  </select>
                </label>
                <div className="plan06Actions">
                  <button type="button" disabled={busy||!current.reviewCurrent||
                    active.needsHumanReconciliation}
                    onClick={()=>void prepareAgentPlan()}>
                    Review source and prepare separate agent draft…
                  </button>
                  <button type="button" disabled={busy}
                    onClick={()=>void reopenAgentLinks()}>
                    Reopen original agent-plan links…
                  </button>
                  <button type="button" disabled={busy}
                    onClick={()=>void refreshEvidence()}>
                    Independently recheck project source for evidence review…
                  </button>
                </div>
                {!current.reviewCurrent&&<small>Review the current task-board order and wording first;
                  stale board approval is not usable as a source lease.</small>}
                {active.needsHumanReconciliation&&<small>Resolve the original source-change
                  reconciliation before creating a new plan draft.</small>}
                <label>Why does this independently observed result support
                  or fail the EXACT acceptance criterion?
                  <textarea rows={3} maxLength={500}
                    aria-label="Operator reason for independently witnessed evidence"
                    placeholder="Explain which part of this criterion the witness does or does not establish…"
                    value={criterionReason}
                    onChange={e=>setCriterionReason(e.target.value)}/>
                </label>
                <small>P0-TOOLS-05 requires at least 10 characters of HUMAN reasoning,
                  a fresh OS-picked project source, reopened original native
                  oracle receipts, and a separate native confirmation.
                  Accepting evidence never completes the task.</small>
                <button type="button" disabled={busy}
                  onClick={()=>void reopenCriterionReviews()}>
                  Reopen original human witness decisions…
                </button>
                {criterionReviews.length>0&&<div role="region"
                  aria-label="Original independently audited human criterion decisions">
                  {criterionReviews.map(row=><article key={row.eventId}>
                    <strong>{row.decision.replaceAll('_',' ')} · task v{row.taskVersion}</strong>
                    <p>{row.operatorReason}</p>
                    <small>Method: {row.independentMethod} · Original receipt:
                      {row.receiptSha256} · Independent source:
                      {row.observedSha256}.</small>
                    <p>{row.sourceLeaseCurrent?
                      'Current task register lease; still historical source evidence.':
                      'Task register/board revision changed; old acceptance is historical only.'}</p>
                    <small>Exact criterion reviewed: {row.acceptanceCriterion}</small>
                    <small>Human evidence decision recorded: YES ·
                      Full task complete: NO · Execution authorized: NO.</small>
                  </article>)}
                </div>}
                {agentLinks.length===0?<p>No agent-plan handoff recorded for this task.</p>:
                  <div role="region" aria-label="Original agent task handoffs">
                    {agentLinks.map(link=><article key={link.handoffId}>
                      <strong>Task v{link.taskVersion} · {link.phase} ·
                        {link.sourceLeaseCurrent?' current register lease':' historical/stale lease'}</strong>
                      <p>Conversation: <code>{link.chatId}</code></p>
                      <p>Agent plan: <code>{link.planId||'Prepared only — manual recovery required'}</code></p>
                      <small>Revision: {link.revisionSha256||'not staged'} ·
                        Human review: {link.planReview} ·
                        Host custody: {link.supervisorState} ·
                        Historical witness: {link.historicalWitnessLinked?'linked, not present-day proof':'none'}.</small>
                      <p>{link.nextSafeAction}</p>
                      {link.independentlyAuditedOutcomes?.length>0&&
                        <div role="region" aria-label="Original independent agent outcome receipts">
                          {link.independentlyAuditedOutcomes.map(result=><article
                            key={result.receiptSha256+':'+result.stepIndex}>
                            <strong>Original step {result.stepIndex+1} · {result.historicalVerdict}</strong>
                            <p>Installed reviewed adapter: {result.toolId} ·
                              Native oracle: {result.independentMethod}</p>
                            <p>{result.criterionMethodReviewable?
                              'Original criterion matches this native method family; this does not prove the task is complete.':
                              'No exact typed criterion-method match: reject relevance or request more evidence.'}</p>
                            <small>Original effect receipt: {result.receiptSha256} ·
                              Independently observed: {result.observedSha256}.</small>
                            <p>{result.reviewEligible?
                              'Eligible for a NEW native PLAN-07 evidence review only; criterion is not accepted.':
                              'Historical/stale or uncertain evidence: reconciliation is required.'}</p>
                            <small>Original requirement: {result.originalRequirementId} ·
                              Full task completed: NO · Execution permission: NO.</small>
                            {result.reviewEligible&&<div className="plan06Actions"
                              aria-label="Native human criterion-witness evidence decision">
                              <button type="button"
                                disabled={busy||!result.criterionMethodReviewable||
                                  criterionReason.trim().length<10||
                                  !current.reviewCurrent}
                                onClick={()=>void reviewIndependentWitness(link,result,
                                  'ACCEPTED_AS_SUPPORTING_EVIDENCE')}>
                                Review and accept as supporting evidence…
                              </button>
                              <button type="button"
                                disabled={busy||criterionReason.trim().length<10}
                                onClick={()=>void reviewIndependentWitness(link,result,
                                  'REJECTED_AS_IRRELEVANT')}>
                                Review and reject relevance…
                              </button>
                              <button type="button"
                                disabled={busy||criterionReason.trim().length<10}
                                onClick={()=>void reviewIndependentWitness(link,result,
                                  'NEEDS_MORE_INDEPENDENT_EVIDENCE')}>
                                Review and request more evidence…
                              </button>
                            </div>}
                          </article>)}
                        </div>}
                      <small>Task complete: NO · Execution authorized by this link: NO.
                        Open the dedicated conversation in Lu to review the agent plan
                        and separately authorize any supported native operation.</small>
                    </article>)}
                  </div>}
              </section>
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
