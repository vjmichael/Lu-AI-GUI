/**
 * Presentation-only recognition of one deterministic OS-clock request while
 * no GGUF model is loaded. The Tauri host independently reclassifies the
 * original user turn and never treats this boolean as execution authority.
 */
export function isHostCurrentTimeQuestion(value){
  if(typeof value!=='string')return false;
  const s=value.trim().toLowerCase();
  if(!s||s.length>256||s.includes('[attached text file:')||s.includes('<|'))return false;
  if(["don't","do not","without","how to","what is time complexity",
      "time travel","time zone difference","timing estimate"].some(term=>s.includes(term)))return false;
  // Presentation-only mirror of Rust's complete-question requirement.
  // A combined clock + tool request must not bypass model readiness.
  const exact=s.replace(/\?+$/,'').trimEnd();
  return ["what time is it","what's the time","what is the current time",
    "what time is it right now","tell me the time",
    "give me the current time","what is my local time",
    "what's the current time","what's the local time"].includes(exact);
}


/**
 * PERF-01 P0D: presentation-only no-model admission for a narrowly typed
 * original-chat exact literal request. The Rust host reclassifies independently.
 * No user-chosen path, stored memory, model output, or snippet becomes a grant.
 */
export function isHostCpuChatLookup(value){
  if(typeof value!=='string')return false;
  const text=value.trim();
  if(!text||text.length>512||text.includes('<|')||
     text.toLowerCase().includes('[attached text file:'))return false;
  const prefix=['Search this chat for: ','Find in this conversation: ']
    .find(p=>text.startsWith(p));
  if(!prefix)return false;
  const q=text.slice(prefix.length);
  return q.length>=2 && q.length<=128 && q.trim()===q &&
    !/[\u0000-\u001f\u007f<>\x60]/.test(q);
}


/** PERF-01 presentation-only status request; Rust revalidates everything. */
export function isHostModelStatusQuestion(value){
  if(typeof value!=='string')return false;
  return ['show local model status','what model is loaded?',
    'is a model loaded?'].includes(value.trim().toLowerCase());
}


/** Presentation-only CPU search admission. Rust reclassifies the original
 * text and obtains a native-confirmed directory handle before reading. */
export function isHostCpuWorkspaceLookup(value){
  if(typeof value!=='string')return false;
  const s=value.trim(), lower=s.toLowerCase();
  const bytes=x=>new TextEncoder().encode(x).length;
  if(bytes(s)>512||!lower.startsWith('search ')||
     /[\u0000-\u001f\u007f-\u009f<>\x60]/.test(s)||lower.includes('[attached text file:'))return false;
  const at=lower.indexOf(' for: ');
  if(at<7)return false;
  const scope=s.slice(7,at).trim(), query=s.slice(at+6);
  const alias=scope.toLowerCase().replace(/^my /,'');
  const aliases=['desktop','desktop folder','documents','documents folder','downloads','downloads folder',
    'this folder','this workspace','this directory','selected workspace','selected folder','selected directory',
    'current folder','current workspace','current directory'];
  let path=scope;
  if(path.startsWith('"')){if(!path.endsWith('"'))return false;path=path.slice(1,-1);}
  if(!aliases.includes(alias)&&(!path||path.includes('"')||!(/^(?:\/|\\\\|[A-Za-z]:)/.test(path))))return false;
  return bytes(query)>=2&&bytes(query)<=128&&query.trim()===query;
}
/** Indexed current-ANSF counts, never cross-chat search or task authority. */
export function isHostCpuChatCountsQuestion(value){
  return typeof value==='string'&&value.trim()==='Show current chat counts';
}

/** Presentation only. Keep the native explicit-no-tools veto in the shared
 * parity fixture; a positive UI affordance never authorizes native dispatch. */
export function isModelFreeSendQuestion(value){
  if(typeof value!=='string')return false;
  const lower=value.toLowerCase();
  if(["no tool","without tools","without any tools","without using tools",
      "do not use tools","don't use tools","do not call tools","don't call tools",
      "without specialists","without any specialists","do not use specialists",
      "never call a tool"].some(term=>lower.includes(term)))return false;
  return isHostCurrentTimeQuestion(value)||isHostCpuChatLookup(value)||
    isHostModelStatusQuestion(value)||isHostCpuWorkspaceLookup(value)||isHostCpuChatCountsQuestion(value);
}
