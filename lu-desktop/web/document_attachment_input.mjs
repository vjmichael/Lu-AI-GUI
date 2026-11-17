// Ordinary file selection, drop and clipboard File objects carry bytes only.
// The native Reader independently checks content, size and supported format.
export const MAX_DOCUMENT_BYTES = 8 * 1024 * 1024;
export const DOCUMENT_EXTENSIONS = Object.freeze([
  'txt','text','md','markdown','log','csv','tsv','json','jsonl','ndjson',
  'xml','html','htm','svg','ini','cfg','conf','rst','properties','env',
  'py','rs','js','jsx','ts','tsx','sql','yaml','yml','toml','go','c','h',
  'cpp','hpp','cs','java','sh','ps1','css','rb','php','swift','kt','lua',
  'docx','xlsx','pptx','ods','odp','odt','rtf','epub','pdf',
]);
export const ATTACHMENT_ACCEPT = ['.png','.jpg','.jpeg',
  ...DOCUMENT_EXTENSIONS.map(ext=>'.'+ext)].join(',');
export function supportedDocumentName(name) {
  const ext=String(name||'').split('.').pop().toLowerCase();
  return DOCUMENT_EXTENSIONS.includes(ext);
}
export async function documentFilePayload(file) {
  if(!file||!supportedDocumentName(file.name))throw Error('Choose a supported document.');
  if(!Number.isSafeInteger(file.size)||file.size<=0||file.size>MAX_DOCUMENT_BYTES)
    throw Error('Documents must be nonempty and no larger than 8 MB.');
  const bytes=new Uint8Array(await file.arrayBuffer());
  if(bytes.length!==file.size)throw Error('The document changed while reading.');
  let binary='';
  for(let i=0;i<bytes.length;i+=16384)
    binary+=String.fromCharCode(...bytes.subarray(i,i+16384));
  return {displayName:file.name,encodedBytes:btoa(binary)};
}
export function documentDraftItem(document) {
  if(!document||typeof document.text!=='string'||!document.text.trim()||
     !/^[0-9a-f]{64}$/.test(document.sourceSha256)||
     !Number.isSafeInteger(document.bytesRead)||document.bytesRead<=0||
     !Number.isSafeInteger(document.offsetChars)||document.offsetChars<0)
    throw Error('Lu could not confirm a readable document excerpt.');
  const label='[Reader extraction: '+document.format+
    (document.selector?'; '+document.selector:'')+
    '; character offset '+document.offsetChars+
    '; source SHA-256 '+document.sourceSha256+']\n';
  const extracted=label+document.text+
    (document.truncated?'\n[Reader: more text may remain; continuation is source-pinned]':'');
  return {name:document.name,size:document.bytesRead,text:extracted,
    format:document.format,truncated:document.truncated,
    sourceSha256:document.sourceSha256,selector:document.selector,
    offsetChars:document.offsetChars};
}
