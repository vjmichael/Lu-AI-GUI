// UX-CHAT01: host validates palettes again. No CSS URLs or raw HTML.
export const MESSAGE_REACTIONS = Object.freeze(['👍','❤️','😂','🎉','😮','👀']);
export const DEFAULT_APPEARANCE = Object.freeze({
  background:'#151e2b',userBubble:'#315b86',assistantBubble:'#1c2a3b'
});
export const CHAT_APPEARANCE_PRESETS = Object.freeze([
  {name:'Original',background:'#151e2b',userBubble:'#315b86',assistantBubble:'#1c2a3b'},
  {name:'Ocean',background:'#132334',userBubble:'#28618b',assistantBubble:'#1e3c53'},
  {name:'Forest',background:'#14251f',userBubble:'#3b654a',assistantBubble:'#263e33'},
  {name:'Light',background:'#f1f3f7',userBubble:'#cbdfff',assistantBubble:'#ffffff'}
]);
export function isSafeHexColor(value){
  return typeof value==='string'&&/^#[a-fA-F0-9]{6}$/.test(value);
}
export function safeAppearance(value){
  if(!value||!['background','userBubble','assistantBubble'].every(
    key=>isSafeHexColor(value[key])))return {...DEFAULT_APPEARANCE};
  return {
    background:value.background,
    userBubble:value.userBubble,
    assistantBubble:value.assistantBubble
  };
}
export function textForBackground(hex){
  if(!isSafeHexColor(hex))return '#f4f7fb';
  const rgb=[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)/255);
  const converted=rgb.map(c=>c<=0.04045?c/12.92:((c+0.055)/1.055)**2.4);
  const lum=0.2126*converted[0]+0.7152*converted[1]+0.0722*converted[2];
  return lum>=0.18?'#142031':'#f5faff';
}
export function visibleReactionBySeq(rows){
  const result={};
  for(const row of rows||[]){
    if(Number.isSafeInteger(row.seq)&&row.seq>0 &&
      MESSAGE_REACTIONS.includes(row.emoji))result[row.seq]=row.emoji;
  }
  return result;
}
