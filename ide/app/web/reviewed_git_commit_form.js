// No authorization comes from this form. Native host still verifies the exact
// stage receipt, index, metadata, and separate operating-system confirmation.
export function readReviewedCommitMetadata(form) {
  if (!form || typeof form.getAll!=='function') return null;
  const result={};
  for (const [name, limit] of [['message',1200], ['authorName',96], ['authorEmail',96]]) {
    const values=form.getAll(name);
    if (values.length!==1 || typeof values[0]!=='string') return null;
    const value=values[0];
    if (!value.trim() || /[\u0000-\u001f\u007f-\u009f]/u.test(value) ||
        new TextEncoder().encode(value).length>limit) return null;
    result[name]=value;
  }
  return result;
}
