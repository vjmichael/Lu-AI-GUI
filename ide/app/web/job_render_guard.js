// P04: no-project is NOT an active native job. Optional-chaining comparisons
// of two missing project IDs evaluate undefined===undefined (true), and the
// protected Job requests view then dereferences absent worker data and makes
// the whole installed WebView UI disappear. Never infer job ownership from
// a missing identity on either side.
export function belongsToSelectedProject(selected, record) {
  return typeof selected?.projectId === 'string' &&
    selected.projectId.length > 0 &&
    typeof record?.projectId === 'string' &&
    record.projectId.length > 0 &&
    record.projectId === selected.projectId;
}
