// View-owned draft metadata. Native host remains the authority for image bytes.
export const MAX_SUBMITTED_PROMPT = 16384;
export const MAX_TEXT_FILES = 4;
export function cleanAttachmentName(name) {
  return String(name||'untitled').replace(/[\\/\x00-\x1f\[\]<>]/g,'_').slice(0,120);
}
export function textFileSection(file) {
  return `\n\n[Attached text file: ${cleanAttachmentName(file.name)}]\n${file.text}\n[/Attached text file]`;
}
export function buildSubmittedPrompt(prompt,files=[]) {
  const value=String(prompt||'').trim()+files.map(textFileSection).join('');
  if (!String(prompt||'').trim()) throw Error('Type a question or instruction before sending.');
  if(new TextEncoder().encode(value).byteLength>MAX_SUBMITTED_PROMPT)
    throw Error('The message and attached text exceed the 16,384-byte model input limit.');
  return value;
}
export function attachmentSummary(file) {
  const size=Math.max(0,Number(file.size)||0);
  return `${cleanAttachmentName(file.name)} · ${size<1024?`${size} B`:`${Math.ceil(size/1024)} KB`}`;
}
