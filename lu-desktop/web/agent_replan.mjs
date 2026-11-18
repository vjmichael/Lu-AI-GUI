// Phase 2: human-editable, bounded replan outline; no tool or authority names.
const kindToOutput = Object.freeze({
  COLLECT_SOURCE:'SOURCE_REFERENCES',
  INSPECT:'STRUCTURED_FINDINGS',
  ANALYZE:'STRUCTURED_FINDINGS',
  INDEPENDENT_CHECK:'INDEPENDENT_WITNESS',
  HUMAN_REVIEW:'HUMAN_DECISION',
  PRESENT:'EXPLANATION'
});
export const REPLAN_REASONS = Object.freeze([
  'CHANGE_APPROACH','CORRECT_RESULT','REVISE_DEPENDENCIES','GOAL_CHANGED'
]);
export function parseHumanReplanOutline(text, previousSteps=[]) {
  if (typeof text !== 'string' || text.length > 512) {
    throw Error('Replacement plan must be a short list of step types');
  }
  const names = text.split(/[,\n]/).map(s=>s.trim().toUpperCase()).filter(Boolean);
  if (names.length<1 || names.length>8) {
    throw Error('Choose one to eight replacement steps');
  }
  if (names.some(name=>!Object.hasOwn(kindToOutput,name))) {
    throw Error('Use the listed step types only; tool names and commands are not a plan');
  }
  const previous = Array.isArray(previousSteps)
    ? previousSteps.map(item=>item?.purpose) : [];
  if (previous.length===names.length && previous.every((name,i)=>name===names[i])) {
    throw Error('Change the plan steps or dependency sequence before creating a replacement');
  }
  return names.map((kind,i)=>({
    kind,dependsOn:i===0?null:i-1,expected:kindToOutput[kind]
  }));
}
export function humanReplanDefaultOutline(steps) {
  return Array.isArray(steps)?steps.map(s=>s?.purpose||'').filter(Boolean).join(', '):'';
}
