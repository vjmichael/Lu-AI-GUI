// Native manifests are authoritative. The composer receives metadata only;
// paths and document excerpt bodies are never part of new linked drafts.
export function mergeAttachmentDraft(native, previous, extra=[]) {
  const cache=new Map([...previous.images,...extra].map(im=>[im.imageRef,im]));
  const images=native.images.map(im=>({...im,linked:false}));
  const files=previous.files.filter(f=>!f.attachmentId);
  native.linked.forEach((a,linkOrder)=>{
    if(a.readerFormat){
      files.push({attachmentId:a.attachmentId,name:a.name,size:a.bytes,available:a.available,
        format:a.readerFormat,selector:a.selector,sourceSha256:a.sourceSha256,
        offsetChars:a.offsetChars,nextOffsetChars:a.nextOffsetChars,sourceTruncated:a.sourceTruncated,linkOrder});
    }else{
      images.push({imageRef:a.attachmentId,name:a.name,mimeType:a.mediaType,size:a.bytes,
        observed:false,available:a.available,linked:true,linkOrder});
    }
  });
  return {prompt:previous.prompt,files,images:images.map(im=>{
    const cached=cache.get(im.imageRef);
    return {...im,...(im.available&&cached?.previewUrl?{previewUrl:cached.previewUrl}:{}),
      ...(cached?.detectedMimeType?{detectedMimeType:cached.detectedMimeType}:{})};
  })};
}
export function attachmentRefs(draft){
  const imageRefs=draft.images.filter(im=>!im.linked).map(im=>im.imageRef);
  const links=[...draft.images.filter(im=>im.linked).map(im=>({id:im.imageRef,order:im.linkOrder})),
    ...draft.files.filter(f=>f.attachmentId).map(f=>({id:f.attachmentId,order:f.linkOrder}))];
  links.sort((a,b)=>a.order-b.order);
  if(links.some((a,i)=>a.order!==i||!(/^(img|doc)-[0-9a-f]{32}$/).test(a.id))||
      new Set([...imageRefs,...links.map(a=>a.id)]).size!==imageRefs.length+links.length){
    throw Error('Attachments changed. Refresh the draft before sending.');
  }
  return {imageRefs,linkedRefs:links.map(a=>a.id)};
}
export function attachedRecoveryMatches(draft,snapshot){
  try{
    const refs=attachmentRefs(draft);
    const equal=(a,b)=>a.length===b.length&&a.every((id,i)=>id===b[i]);
    return equal(refs.imageRefs,snapshot.imageRefs)&&equal(refs.linkedRefs,snapshot.linkedRefs||[]);
  }catch(_){return false;}
}
export function attachmentSubmissionText(draft){
  if(!draft.prompt.trim())throw Error('Type a question or instruction before sending.');
  if(draft.files.some(f=>!f.attachmentId))throw Error('Attach the original document instead of an old copied excerpt.');
  if([...draft.images,...draft.files].some(a=>a.available===false))throw Error('File not available. Remove or attach the original file again.');
  const refs=attachmentRefs(draft);
  const text=refs.linkedRefs.length?draft.prompt:draft.prompt.trim();
  if(new TextEncoder().encode(text).length>16384)throw Error('Message exceeds the 16 KiB text limit.');
  return text;
}
