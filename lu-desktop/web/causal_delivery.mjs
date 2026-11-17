// CP-01E: no text matching or claim of paint. The committed row must
// correspond to the exact native assistant sequence and original turn.
export function deliveryCandidate({witness,chatId,activeTab,currentChat,job,entries,hasCommittedNode}){
  if(!witness||activeTab!=='chat'||chatId!==witness.chatId||currentChat!==chatId||
     job?.generationId===witness.generationId||!Number.isSafeInteger(witness.assistantSeq)||
     witness.assistantSeq<=0||!hasCommittedNode)return false;
  return entries.some(row=>row.seq===witness.assistantSeq&&row.role==='assistant'&&
    row.interrupted===false&&row.cancelled===false&&typeof row.text==='string'&&row.text.length>0);
}
