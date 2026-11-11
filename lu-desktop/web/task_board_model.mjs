// PLAN-06 presentation only. Native host owns original evidence, status,
// versioning, human review, execution authority and persisted ordering.
export const FILTERS=Object.freeze(['All','Broken','Not integrated','Unverified','Completed']);
export function taskKind(t){
  if(t?.independentlyVerifiedRequirement===true)return 'Completed';
  if(t?.observedStatus==='BROKEN')return 'Broken';
  if(t?.observedStatus==='NOT_INTEGRATED')return 'Not integrated';
  return 'Unverified';
}
export function taskStatus(t){
  if(t?.needsHumanReconciliation)return 'Needs human source/wording review';
  if(t?.independentlyVerifiedRequirement===true)
    return 'Exact requirement verified · full task still unproven';
  return String(t?.observedStatus||'UNKNOWN').replaceAll('_',' ').toLowerCase()+
    ' · not independently accepted';
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
