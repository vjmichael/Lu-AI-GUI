// Explicit local folder intent is UI routing only, NEVER a filesystem grant.
// A native per-turn OS directory picker still admits the reviewed workspace.
export function routeExplicitLocalFolderRequest(value){
  if(typeof value!=='string'||value.length>8192)return false;
  const text=value.trim();
  if(!text||text.includes('[Attached text file:')||text.includes('<|'))return false;
  if(/\b(?:do not|don't|never|without)\s+(?:open|navigate|browse|inspect|list|search|explore)\b/i.test(text))return false;
  const explicitVerb=/\b(?:open|inspect|navigate\s+to|go\s+to|browse|explore|look\s+in|search\s+in|list\s+(?:files\s+)?in|show\s+(?:the\s+)?contents\s+of)\b/i;
  if(!explicitVerb.test(text.slice(0,140)))return false;
  const drive=/(?:^|[\s"'])(?:[A-Za-z]:\\)(?:[^<>\r\n]*)/;
  const unc=/(?:^|[\s"'])(?:\\\\[A-Za-z0-9._-]+\\[A-Za-z0-9._$-]+)/;
  return drive.test(text)||unc.test(text);
}
