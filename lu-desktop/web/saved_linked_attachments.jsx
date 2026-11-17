import React,{useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';

// An availability result is an observation, never a durable grant. A
// click always reopens the exact original, even if a check just succeeded.
export function SavedLinkedAttachments({chatId,visibleSeq,attachments}){
  const [availability,setAvailability]=useState({});
  const [preview,setPreview]=useState(null);
  const [failure,setFailure]=useState(null);
  const root=useRef(null);
  const active=useRef(false);
  const availabilityRequest=useRef(0);
  const clickRequest=useRef(0);
  useEffect(()=>{
    active.current=true;
    let visible=false;
    const check=async()=>{
      const request=++availabilityRequest.current;
      try{
        const rows=await invoke('lu_linked_attachment_status',{chatId,visibleSeq});
        if(!active.current||availabilityRequest.current!==request)return;
        setAvailability(Object.fromEntries(rows.map(row=>[row.attachmentId,row.available===true])));
      }catch{/* Unknown is not available or missing. The click independently checks. */}
    };
    const focus=()=>{if(visible)void check();};
    const observer=typeof IntersectionObserver==='function'
      ?new IntersectionObserver(entries=>{
         const now=entries.some(e=>e.isIntersecting);
         if(now&&!visible)void check();
         visible=now;
       }):null;
    if(observer&&root.current)observer.observe(root.current);
    else{visible=true;void check();}
    window.addEventListener('focus',focus);
    return ()=>{
      active.current=false;availabilityRequest.current++;clickRequest.current++;
      observer?.disconnect();window.removeEventListener('focus',focus);
    };
  },[chatId,visibleSeq]);
  async function open(link){
    const request=++clickRequest.current;
    availabilityRequest.current++;
    setPreview(null);setFailure(null);
    try{
      const result=await invoke('lu_read_linked_attachment',{
        chatId,visibleSeq,attachmentId:link.attachmentId});
      if(!active.current||request!==clickRequest.current)return;
      if(result?.kind!=='document'&&result?.kind!=='image')throw Error('File not available.');
      setAvailability(old=>({...old,[link.attachmentId]:true}));
      setPreview({name:link.name,...result});
    }catch{
      if(!active.current||request!==clickRequest.current)return;
      setAvailability(old=>({...old,[link.attachmentId]:false}));
      setFailure(link.name+': File not available. It may have been moved, deleted or changed.');
    }
  }
  return <section ref={root} className="linkedAttachmentArea" aria-label="Linked files">
    <div className="savedAttachments">
      {attachments.map(link=><button type="button" key={link.attachmentId}
        className="attachment linkedAttachment" onClick={()=>void open(link)}>
        <span aria-hidden="true">▤</span><span>{link.name}</span>
        {availability[link.attachmentId]===false&&<small>File not available</small>}
      </button>)}
    </div>
    {failure&&<p role="status" className="linkedAttachmentFailure">{failure}</p>}
    {preview&&<div className="linkedAttachmentPreview">
      <header><strong>{preview.name}</strong><button type="button" className="subtleButton"
        onClick={()=>{clickRequest.current++;setPreview(null);}}>Close</button></header>
      {preview.kind==='image'?<img src={preview.dataUrl} alt={preview.name}/>:
        <><pre>{preview.document.text}</pre>
          {preview.document.truncated&&<small>Showing part of this document.</small>}</>}
    </div>}
  </section>;
}
