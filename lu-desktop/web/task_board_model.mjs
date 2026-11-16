// PLAN-06 presentation only. Native host owns original evidence, status,
// versioning, human review, execution authority and persisted ordering.
export const FILTERS=Object.freeze(['All','Broken','Not integrated','Unverified','Requirement verified']);
export function taskKind(t){
  // Source requirement verification is NOT whole-task completion. A current
  // native folder scan and independent human completion decision live in the
  // separate C3C authority journal, never in a model/board classification.
  if(t?.independentlyVerifiedRequirement===true&&!t?.needsHumanReconciliation)
    return 'Requirement verified';
  if(t?.observedStatus==='BROKEN')return 'Broken';
  if(t?.observedStatus==='NOT_INTEGRATED')return 'Not integrated';
  return 'Unverified';
}
export function taskStatus(t){
  if(t?.needsHumanReconciliation)return 'Needs human source/wording review';
  if(t?.independentlyVerifiedRequirement===true)
    return 'Exact native requirement verified · whole task still requires independent completion decision';
  return String(t?.observedStatus||'UNKNOWN').replaceAll('_',' ').toLowerCase()+
    ' · not independently accepted';
}
export function reconciledTaskStatus(task,decision){
  if(task&&decision&&decision.taskId===task.taskId&&
     decision.taskVersion===task.version&&
     decision.taskCompleted===true&&
     decision.sourceRescannedThisReview===true&&
     decision.status==='WHOLE_TASK_HUMAN_DECISION_CURRENT_SOURCE_REVERIFIED'&&
     decision.executionAuthorized===false)
    return 'Whole task completed · original human decision and current native source independently reverified';
  return taskStatus(task);
}
// Pure UI admission only. The native command still reselects and rescans
// the source and validates original worker, dependency and decision history
// after a SECOND OS-owned human confirmation. A reloaded window has no
// reconciliation object and cannot reuse a previous app session's permission.
export function canRequestPositiveWholeTask(task,board,review){
  return !!task&&board?.reviewCurrent===true&&
    task.needsHumanReconciliation===false&&
    review?.taskId===task.taskId&&review?.taskVersion===task.version&&
    review.sourceRescannedThisReview===true&&
    review.nativeRequirementAccepted===true&&
    review.historyFullyInspected===true&&
    review.allDeclaredCriteriaIndependentlyVerified===true&&
    review.requiredCriterionCount>0&&
    review.executionAuthorized===false&&review.taskCompleted===false&&
    review.status==='ALL_CRITERIA_VERIFIED_AWAITING_SEPARATE_HUMAN_TASK_DECISION';
}
export function shownTasks(tasks,filter='All',search=''){
  const needle=String(search).trim().toLowerCase();
  return tasks.filter(t=>(filter==='All'||taskKind(t)===filter)&&
    (!needle||[t.summary,t.description,t.group,t.sourceRequirementId]
      .some(s=>String(s).toLowerCase().includes(needle))));
}
export function taskGroups(tasks){
  return tasks.reduce((groups,task)=>{
    let group=groups.find(g=>g.name===task.group);
    if(!group){group={name:task.group,tasks:[]};groups.push(group);}
    group.tasks.push(task);return groups;
  },[]);
}
export function movedOrder(ids,taskId,delta){
  if(!Number.isInteger(delta)||Math.abs(delta)!==1)return null;
  const order=[...ids],at=order.indexOf(taskId),to=at+delta;
  if(at<0||to<0||to>=order.length)return null;
  [order[at],order[to]]=[order[to],order[at]];
  return order;
}
