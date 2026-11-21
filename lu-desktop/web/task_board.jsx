import React,{useEffect,useMemo,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {NativeWindowFrame} from './native_windows.jsx';
import {friendlyError} from './unified_tabs.jsx';
import {FILTERS,taskKind,taskStatus,reconciledTaskStatus,canRequestPositiveWholeTask,shownTasks,taskGroups,movedOrder} from './task_board_model.mjs';

const errorText=e=>String(e?.message||e||'Native task list unavailable');
const correction=t=>({
  summary:t.summary,description:t.description,priority:t.priority,
  blockedBy:t.blockedBy,acceptanceCriteria:t.acceptanceCriteria,reason:''
});
export function TaskBoardWindow({onNavigate}={}){
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
  const [completionReconciliation,setCompletionReconciliation]=useState(null);
  const [completionHistory,setCompletionHistory]=useState([]);
  const [wholeDecision,setWholeDecision]=useState('BLOCKED');
  const [wholeReason,setWholeReason]=useState('');
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
    setCompletionReconciliation(null);
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
    setCompletionReconciliation(null);setCompletionHistory([]);
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
    setCompletionReconciliation(null);setCompletionHistory([]);
    try{
      const [detail,links,reviews,completions]=await Promise.all([
        invoke('lu_project_board_evidence',{
          registerId:current.board.registerId,taskId:t.taskId}),
        invoke('lu_project_task_agent_handoffs',{
          registerId:current.board.registerId,taskId:t.taskId}),
        invoke('lu_project_task_criterion_evidence_history',{
          registerId:current.board.registerId,taskId:t.taskId}),
        invoke('lu_project_task_completion_history',{
          registerId:current.board.registerId,taskId:t.taskId})
      ]);
      setEvidence(detail);setAgentLinks(links);setCriterionReviews(reviews);
      setCompletionHistory(completions);
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
  async function reconcileTaskCompletion(){
    if(!current||!active||busy)return;
    await act('Reopened original native source and independent evidence. No task completion or future tool grant was recorded.',async()=>{
      // This native command requires its OWN OS folder picker and rescan.
      // A model prompt, cached path or prior human decision is not accepted.
      const summary=await invoke('lu_project_task_reconcile_completion',{
        registerId:current.board.registerId,taskId:active.taskId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256,
        expectedTaskVersion:active.version
      });
      if(summary)setCompletionReconciliation(summary);
      return summary;
    });
  }
  async function decideWholeTask(){
    if(!current||!active||busy||wholeReason.trim().length<10||
       (wholeDecision==='COMPLETED_AT_REVIEW'&&
        !canRequestPositiveWholeTask(active,current,completionReconciliation)))return;
    await act('Original whole-task decision written and independently journaled. No future tool execution authorized.',async()=>{
      const result=await invoke('lu_project_task_decide_completion',{
        registerId:current.board.registerId,taskId:active.taskId,
        expectedBoardSha256:current.board.sha256,
        expectedRegisterSha256:current.sourceRegisterSha256,
        expectedTaskVersion:active.version,
        decision:wholeDecision,operatorReason:wholeReason.trim()
      });
      if(result){
        const rows=await invoke('lu_project_task_completion_history',{
          registerId:current.board.registerId,taskId:active.taskId
        });
        setCompletionHistory(rows);
        setCompletionReconciliation(null);
        setWholeReason('');
      }
      return result;
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
    await act('Review saved. Task status and permissions are unchanged.',async()=>{
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
        setCriterionReason('');setCompletionReconciliation(null);
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
      await act('Model suggestions saved for review. No tasks were accepted or run.',async()=>{
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
    await act('Saved model suggestions reopened and checked.',async()=>{
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
      setError('Reopen the matching saved model suggestion before reviewing it.');
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
  if(onNavigate)return <NativeWindowFrame onNavigate={onNavigate} title="Project plans" subtitle="Review and manage your work">
    <div className="nativeModelContent" aria-label="Project plans">
      <div className="nativeChoiceRow">
        <button className="subtleButton" disabled={busy} onClick={()=>void act('Projects refreshed.',reload)}>Refresh projects</button>
        {current&&<button className="primaryButton" disabled={busy||current.reviewCurrent} onClick={()=>void approve()}>Review plan…</button>}
      </div>
      {error&&<p role="alert" className="nativeError">{friendlyError(error)}</p>}
      {notice&&<p role="status" className="nativeNotice">Updated.</p>}
      {!boards.length&&<p>No saved project plans yet.</p>}
      {boards.length>0&&<div className="nativeChoiceRow">
        {boards.map((board,index)=><button type="button" key={board.board.registerId} aria-pressed={current?.board.registerId===board.board.registerId} onClick={()=>setRegisterId(board.board.registerId)}>
          Project {index+1} · {board.tasks.length} tasks
        </button>)}
      </div>}
      {current&&<><h2>{current.reviewCurrent?'Reviewed plan':'Draft plan'}</h2>
        <label className="nativeField">Search tasks<input type="search" value={search} onChange={e=>setSearch(e.target.value)}/></label>
        {shown.map(task=><article className="nativeOption" key={task.taskId}>
          <button className="subtleButton" type="button" onClick={()=>void selectTask(task)}>{task.summary}</button>
          <p>{task.needsHumanReconciliation?'Needs review':task.independentlyVerifiedRequirement?'Requirement checked; completion not confirmed':task.observedStatus==='BROKEN'?'Needs repair':'Not yet verified'}</p>
          <div className="nativeChoiceRow">
            <button type="button" disabled={busy||Boolean(search.trim())||all[0]?.taskId===task.taskId} onClick={()=>void reorder(task,-1)}>Move up</button>
            <button type="button" disabled={busy||Boolean(search.trim())||all.at(-1)?.taskId===task.taskId} onClick={()=>void reorder(task,1)}>Move down</button>
          </div>
        </article>)}
        {active&&<section className="nativeOption"><h2>{active.summary}</h2><p>{active.description}</p>
          <p><strong>Completion requirement:</strong> {active.acceptanceCriteria}</p>
          {active.blockedBy&&<p><strong>Blocked by:</strong> {active.blockedBy}</p>}
          <button type="button" disabled={busy||!current.reviewCurrent||active.needsHumanReconciliation} onClick={()=>void prepareAgentPlan()}>Prepare a reviewed plan…</button>
          <button type="button" disabled={busy} onClick={()=>void exportPlan()}>Export plan…</button>
        </section>}
      </>}
    </div>
  </NativeWindowFrame>;
  return <NativeWindowFrame onNavigate={onNavigate} title="Project Tasks"
    subtitle={onNavigate?"Review and manage project plans":"Human-reviewed plans · original evidence · no model execution"}>
    <div className="plan06Layout" aria-label="Project task list">
      <header className="plan06Toolbar">
        <label>Project
          <select aria-label="Project selector" disabled={busy}
            value={current?.board.registerId||''}
            onChange={e=>setRegisterId(e.target.value)}>
            {boards.map((b,index)=><option key={b.board.registerId} value={b.board.registerId}>
              Project {index+1} · {b.tasks.length} tasks
            </option>)}
          </select>
        </label>
        <button type="button" disabled={busy}
          onClick={()=>void act('Project lists refreshed.',reload)}>Refresh registers</button>
        {current&&<span role="status" className="plan06State">
          {current.reviewCurrent?'Human-reviewed plan · NOT executable':
            current.board.reviewState==='human_approved'?'Prior approval stale after task revisions':
            'Draft · no plan approval'}
        </span>}
      </header>
      {error&&<p role="alert" className="nativeError">{error}</p>}
      {notice&&<p role="status" className="nativeNotice">{notice}</p>}
      {!current?<p className="plan06Empty">No project tasks yet. Open Settings to set up a project plan.
        Reviewing a task does not grant access to files or run tools.</p>:<>
        <details className="plan06SourceRecords" aria-label="Original project source records">
          <summary>Original source records</summary>
        <div className="plan06Provenance">
          <small>Original proposal: {current.originalProposalSha256}</small>
          <small>Source register: {current.sourceRegisterSha256}</small>
          <small>Plan revision {current.board.sequence} · {current.eventCount} task-history events · {current.historicalApprovalCount} historical plan reviews</small>
        </div>
        </details>
        <section className="plan07Refresh" aria-label="Evidence-driven task update review">
          <div className="plan07RefreshHeader">
            <strong>Check project changes</strong>
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
              <h3>Suggestions from your local model</h3>
              <p>Optional original-model suggestions only. Native citations and status
                are checked by the host; no result completes or executes a task.</p>
              <div className="plan06Actions">
                <button type="button" disabled={busy} onClick={()=>void suggestUpdatesWithGGUF()}>
                  Ask the loaded model for suggestions…
                </button>
                <button type="button" disabled={busy}
                  onClick={()=>void reloadOriginalModelReceipt()}>
                  Review saved model suggestions…
                </button>
                <button type="button" disabled={!modelBusy||modelCancelBusy}
                  onClick={()=>void cancelGgufSuggestions()}>
                  {modelCancelBusy?'Stopping model suggestions…':'Stop model suggestions…'}
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
            <h3>Review proposed project changes</h3>
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
                onChange={()=>setReviewMode('model')}/> Model suggestion (not independently verified)</label>
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
            {reviewHistory.length===0?<small>No project change reviews recorded yet.</small>:
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
                      <small>Revision {task.version}</small>
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
              <p><strong>Classification:</strong> {reconciledTaskStatus(active,completionReconciliation)}</p>
              <p><strong>Acceptance criterion:</strong> {active.acceptanceCriteria}</p>
              <p><strong>Blocker:</strong> {active.blockedBy}</p>
              <section className="plan06AgentHandoff" aria-label="Selected task agent planning handoff">
                <h3>Prepare a plan for this task</h3>
                <p>This prepares a separate saved conversation and a plan for your review
                  from the original task and a freshly selected native project source.
                  Task-board approval is not tool approval; the task is never marked complete.</p>
                <label>Agent draft mode
                  <select aria-label="Agent task plan mode" value={agentMode}
                    disabled={busy} onChange={e=>setAgentMode(e.target.value)}>
                    <option value="review_only">Review-only source, analysis and human decision (no installed worker adapter)</option>
                    <option value="reviewed_git_media_check">One independently reviewed Git/media source check (separate native tool approval required)</option>
                    <option value="reviewed_whole_task" disabled={!active.acceptanceCriteria?.startsWith('LU_COMPLETE_V2:ALL_OF')}>Typed whole task: separate reviewed native check for each declared Git/media method</option>
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
                  <button type="button" disabled={busy||!current.reviewCurrent||
                    active.needsHumanReconciliation}
                    onClick={()=>void reconcileTaskCompletion()}>
                    Reconcile native requirement and original completion evidence…
                  </button>
                  <button type="button" disabled={busy}
                    onClick={()=>void refreshEvidence()}>
                    Independently recheck project source for evidence review…
                  </button>
                </div>
                {completionReconciliation&&<div role="region"
                  aria-label="Native source and task completion reconciliation">
                  <strong>Native source and completion evidence · {completionReconciliation.status.replaceAll('_',' ')}</strong>
                  <p>Original project rescanned: {completionReconciliation.sourceRescannedThisReview?'yes':'no'} ·
                    Native requirement accepted: {completionReconciliation.nativeRequirementAccepted?'yes':'no'} ·
                    Original independently reopened, human-reviewed witnesses: {completionReconciliation.reopenedHumanCriterionWitnesses}.</p>
                  <p>Declared bounded criteria: {completionReconciliation.requiredCriterionCount} ·
                    Every declared criterion independently verified: {completionReconciliation.allDeclaredCriteriaIndependentlyVerified?'yes':'no'}.
                    A verified criterion set does not itself complete a task.</p>
                  <small>Original human evidence decisions: {completionReconciliation.originalHumanReviewCount} ·
                    Entire review history inspected: {completionReconciliation.historyFullyInspected?'yes':'no'} ·
                    Whole-task decision events: {completionReconciliation.completionDecisionCount} ·
                    Full task completed after fresh independent source recheck: {completionReconciliation.taskCompleted?'yes':'no'} ·
                    Future execution authorized: {completionReconciliation.executionAuthorized?'yes':'no'}.</small>
                </div>}

                <section className="plan06CompletionDecision" aria-label="Separately authorized whole-task completion decision">
                  <h3>Whole-task completion · independent source audit</h3>
                  <p>Whole-task completion requires every exact declared machine criterion,
                    current source and dependencies, then a separate OS-native human
                    approval. Supported bounded criteria are V1 current native source,
                    V1 exact Git HEAD, or canonical <code>LU_COMPLETE_V2:ALL_OF</code>
                    source and independently reviewed Git/media checks. A reviewed
                    Git status or WAV/MP4 fact is evidence of observation, not proof
                    of a clean tree, correct film or completed delivery. Missing,
                    rejected, stale, unsupported or free-text conditions remain
                    incomplete. Blocked, uncertain and reviewed-replan decisions grant
                    no execution authority; an old model or worker answer cannot
                    complete a task.</p>
                  <label>Human whole-task decision
                    <select aria-label="Whole task completion disposition"
                      disabled={busy} value={wholeDecision}
                      onChange={e=>setWholeDecision(e.target.value)}>
                      <option value="BLOCKED">Blocked — incomplete independent proof</option>
                      <option value="UNCERTAIN">Uncertain — reconcile after interruption or source drift</option>
                      <option value="REPLAN_REQUIRED">Reviewed replan required — original criteria not satisfied</option>
                      <option value="COMPLETED_AT_REVIEW">Complete — only if every exact machine criterion and dependency verifies</option>
                    </select>
                  </label>
                  <label>Operator reason for exact task decision
                    <textarea aria-label="Whole task human decision reason"
                      rows={3} maxLength={500} value={wholeReason}
                      onChange={e=>setWholeReason(e.target.value)}
                      placeholder="State how the current original task criterion is or is not satisfied…" />
                  </label>
                  <button type="button" disabled={busy||wholeReason.trim().length<10||
                    (wholeDecision==='COMPLETED_AT_REVIEW'&&
                      !canRequestPositiveWholeTask(active,current,completionReconciliation))}
                    onClick={()=>void decideWholeTask()}>
                    Open native human whole-task decision…
                  </button>
                  {wholeDecision==='COMPLETED_AT_REVIEW'&&
                    !canRequestPositiveWholeTask(active,current,completionReconciliation)&&
                    <p role="status">First select “Reconcile native requirement and original completion evidence” above.
                      Incomplete, cancelled, stale, interrupted or replanned work cannot request
                      positive completion. After restarting Lu, select the source and
                      independently reconcile it again before a separate native decision.</p>}
                  <p><small>Positive completion reselects the source with the OS picker,
                    verifies native evidence and dependencies twice, and commits an
                    immutable human-approved record. Historical completion must be
                    reopened against current source; it grants no execution.</small></p>
                  {completionHistory.length>0&&<div role="region" aria-label="Original whole-task completion decision history">
                    {completionHistory.map(row=><p key={row.eventId}>
                      <strong>{row.decision.replaceAll('_',' ')} · task v{row.taskVersion}</strong>
                      {' · '}{row.operatorReason}
                      {' · '}Historical completion at observation: {row.taskCompletedAtObservation?'yes':'no'}.
                      {' '}Current source rechecked by history view: no.
                    </p>)}
                  </div>}
                </section>
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
                <small>This review requires at least 10 characters explaining your decision,
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
              onClick={()=>void exportPlan()}>Export reviewed project plan…</button>
          </div>
          <small>Review never grants file writes, Git operations, worker scheduling,
            native tools or task completion. Those require separate authorization and evidence.</small>
        </footer>
      </>}
    </div>
  </NativeWindowFrame>;
}
