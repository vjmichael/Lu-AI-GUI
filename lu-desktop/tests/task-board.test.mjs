import test from 'node:test';
import assert from 'node:assert/strict';
import {FILTERS,taskKind,taskStatus,reconciledTaskStatus,canRequestPositiveWholeTask,shownTasks,taskGroups,movedOrder}
  from '../web/task_board_model.mjs';

const tasks=[
  {taskId:'a',summary:'Fix original failure',description:'Source check',
   group:'Core',sourceRequirementId:'REQ-A',observedStatus:'BROKEN',
   independentlyVerifiedRequirement:false,needsHumanReconciliation:false},
  {taskId:'b',summary:'Integrate branch',description:'Git branch',
   group:'Core',sourceRequirementId:'REQ-B',observedStatus:'NOT_INTEGRATED',
   independentlyVerifiedRequirement:false,needsHumanReconciliation:false},
  {taskId:'c',summary:'Inspect source',description:'Source path',
   group:'Evidence',sourceRequirementId:'REQ-C',observedStatus:'ACCEPTED',
   independentlyVerifiedRequirement:false,needsHumanReconciliation:false},
  {taskId:'d',summary:'Check exact requirement',description:'Native proof',
   group:'Evidence',sourceRequirementId:'REQ-D',observedStatus:'ACCEPTED',
   independentlyVerifiedRequirement:true,needsHumanReconciliation:false},
  {taskId:'e',summary:'Needs re-review',description:'Source changed',
   group:'Core',sourceRequirementId:'REQ-E',observedStatus:'ACCEPTED',
   independentlyVerifiedRequirement:true,needsHumanReconciliation:true}
];
test('task list filters do not reinterpret claimed or stale ACCEPTED as completion',()=>{
  assert.deepEqual(FILTERS,['All','Broken','Not integrated','Unverified','Requirement verified']);
  assert.equal(taskKind(tasks[2]),'Unverified');
  assert.equal(taskKind(tasks[3]),'Requirement verified');
  assert.match(taskStatus(tasks[3]),/whole task still requires independent completion decision/);
  assert.match(taskStatus(tasks[4]),/Needs human/);
  assert.deepEqual(shownTasks(tasks,'Broken','').map(x=>x.taskId),['a']);
  assert.deepEqual(shownTasks(tasks,'Not integrated','').map(x=>x.taskId),['b']);
  assert.deepEqual(shownTasks(tasks,'Unverified','').map(x=>x.taskId),['c','e']);
  assert.equal(taskKind(tasks[4]),'Unverified');
  assert.deepEqual(shownTasks(tasks,'Requirement verified','').map(x=>x.taskId),['d']);
  assert.deepEqual(shownTasks(tasks,'Completed',''),[],
    'the board must never infer independently audited whole-task completion');
  assert.deepEqual(shownTasks(tasks,'All','source').map(x=>x.taskId),['a','c','e']);
});
test('only same-version independently reconciled source and human decision changes task status',()=>{
  const t={...tasks[3],version:4};
  const current={taskId:t.taskId,taskVersion:4,taskCompleted:true,
    sourceRescannedThisReview:true,executionAuthorized:false,
    status:'WHOLE_TASK_HUMAN_DECISION_CURRENT_SOURCE_REVERIFIED'};
  assert.match(reconciledTaskStatus(t,current),/Whole task completed/);
  assert.match(reconciledTaskStatus(t,{...current,taskVersion:3}),/whole task still requires/);
  assert.match(reconciledTaskStatus(t,{...current,sourceRescannedThisReview:false}),/whole task still requires/);
  assert.match(reconciledTaskStatus(t,{...current,executionAuthorized:true}),/whole task still requires/);
  assert.match(reconciledTaskStatus(t,{...current,status:'MODEL_FINAL_ANSWER'}),/whole task still requires/);
  assert.match(reconciledTaskStatus(t,{...current,taskCompleted:false}),/whole task still requires/);
});
test('task group rendering preserves human ordered list and respects original group',()=>{
  const groups=taskGroups(tasks);
  assert.deepEqual(groups.map(x=>x.name),['Core','Evidence']);
  assert.deepEqual(groups[0].tasks.map(x=>x.taskId),['a','b','e']);
  assert.deepEqual(groups[1].tasks.map(x=>x.taskId),['c','d']);
});
test('accessible up/down ordering never inserts or drops task identities',()=>{
  const ids=tasks.map(t=>t.taskId);
  assert.deepEqual(movedOrder(ids,'b',1),['a','c','b','d','e']);
  assert.deepEqual(ids,['a','b','c','d','e']);
  assert.deepEqual(movedOrder(ids,'b',-1),['b','a','c','d','e']);
  assert.equal(movedOrder(ids,'a',-1),null);
  assert.equal(movedOrder(ids,'e',1),null);
  assert.equal(movedOrder(ids,'not-a-task',1),null);
  assert.equal(movedOrder(ids,'b',2),null);
});

test('C3 native positive completion cannot bypass fresh same-task full-source reconciliation after restart',()=>{
  const task={...tasks[3],version:4};
  const board={reviewCurrent:true};
  const accepted={
    taskId:task.taskId,taskVersion:4,sourceRescannedThisReview:true,
    nativeRequirementAccepted:true,historyFullyInspected:true,
    allDeclaredCriteriaIndependentlyVerified:true,requiredCriterionCount:3,
    taskCompleted:false,executionAuthorized:false,
    status:'ALL_CRITERIA_VERIFIED_AWAITING_SEPARATE_HUMAN_TASK_DECISION'
  };
  assert.equal(canRequestPositiveWholeTask(task,board,accepted),true,
    'complete machine evidence only opens a separate human OS decision');
  assert.equal(canRequestPositiveWholeTask(task,board,null),false,
    'restart clears the in-memory native source review');
  for(const bad of [
    {...accepted,taskId:'another'},
    {...accepted,taskVersion:3},
    {...accepted,sourceRescannedThisReview:false},
    {...accepted,nativeRequirementAccepted:false},
    {...accepted,historyFullyInspected:false},
    {...accepted,allDeclaredCriteriaIndependentlyVerified:false},
    {...accepted,requiredCriterionCount:0},
    {...accepted,taskCompleted:true},
    {...accepted,executionAuthorized:true},
    {...accepted,status:'REQUIRES_EVERY_DECLARED_CRITERION_INDEPENDENT_PROOF'},
    {...accepted,status:'REQUIRES_NEW_REVIEWED_TASK_REVISION_AFTER_INTERRUPTION'},
    {...accepted,status:'WHOLE_TASK_HUMAN_DECISION_CURRENT_SOURCE_REVERIFIED'},
  ])assert.equal(canRequestPositiveWholeTask(task,board,bad),false,
    'any old, incomplete, cancellation or suspect review must refuse positive UI admission');
  assert.equal(canRequestPositiveWholeTask(task,{reviewCurrent:false},accepted),false);
  assert.equal(canRequestPositiveWholeTask({...task,needsHumanReconciliation:true},board,accepted),false);
});
