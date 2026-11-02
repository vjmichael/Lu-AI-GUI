// Lu-owned, dependency-free Markdown AST. It NEVER interprets raw HTML or CSS.
// React renders its text nodes with ordinary escaping; only http(s) links are active.
const FENCE = /^ {0,3}(`{3,}|~{3,})([a-zA-Z0-9_+#.-]*)\s*$/;
const HEADING = /^ {0,3}(#{1,4})\s+(.+)$/;
const BULLET = /^ {0,3}[-*+]\s+(.+)$/;
const ORDERED = /^ {0,3}(\d{1,3})\.\s+(.+)$/;
const QUOTE = /^ {0,3}>\s?(.*)$/;
const RULE = /^ {0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/;

export function safeMarkdownHref(value) {
  if (typeof value !== 'string' || value.length > 2048) return null;
  // Require an absolute HTTP(S) URL. Never permit file:, data:, javascript:,
  // drive-relative links or WebView custom protocols in model-generated text.
  if (!/^https?:\/\//i.test(value)) return null;
  try {
    const url = new URL(value);
    if (!['https:', 'http:'].includes(url.protocol) || !url.hostname || url.username || url.password)
      return null;
    return url.href;
  } catch { return null; }
}

// Inline nodes deliberately support only a small, predictable subset.
// Unknown or unfinished syntax remains inert, visible text while streaming.
export function parseMarkdownInline(source, depth = 0) {
  const text = String(source ?? '');
  const out = [];
  let plain = '';
  const flush = () => {if (plain) {out.push({type:'text',value:plain});plain='';}};
  let i = 0;
  while (i < text.length) {
    if (text[i] === '\\' && i+1 < text.length && /[\\`*_\[\]()]/.test(text[i+1])) {
      plain += text[i+1]; i+=2; continue;
    }
    if (text[i] === '`') {
      const end = text.indexOf('`',i+1);
      if (end > i+1) {flush();out.push({type:'code',value:text.slice(i+1,end)});i=end+1;continue;}
    }
    if (text[i] === '[') {
      const m = /^\[([^\]\n]{1,200})\]\(([^\s)]+)\)/.exec(text.slice(i));
      if (m) {
        const href = safeMarkdownHref(m[2]);
        if (href) {flush();out.push({type:'link',href,children:parseMarkdownInline(m[1],depth+1)});i+=m[0].length;continue;}
      }
    }
    if (depth < 3 && text.slice(i,i+2)==='**') {
      const end = text.indexOf('**',i+2);
      if (end>i+2) {flush();out.push({type:'strong',children:parseMarkdownInline(text.slice(i+2,end),depth+1)});i=end+2;continue;}
    }
    if (depth < 3 && (text[i] === '*' || text[i] === '_') && text[i+1] !== text[i]) {
      const mark = text[i];
      const end = text.indexOf(mark,i+1);
      if (end>i+1 && text[i+1] !== ' ' && text[end-1] !== ' ') {
        flush();out.push({type:'em',children:parseMarkdownInline(text.slice(i+1,end),depth+1)});i=end+1;continue;
      }
    }
    plain+=text[i];i++;
  }
  flush();
  return out;
}

function cells(line) {
  let trimmed = line.trim();
  if (trimmed.startsWith('|')) trimmed=trimmed.slice(1);
  if (trimmed.endsWith('|')) trimmed=trimmed.slice(0,-1);
  return trimmed.split(/(?<!\\)\|/).map(s=>s.replace(/\\\|/g,'|').trim());
}
function tableBreak(line) {return /^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(line);}
function special(line) {
  return FENCE.test(line) || HEADING.test(line) || RULE.test(line) || BULLET.test(line) || ORDERED.test(line) || QUOTE.test(line);
}
export function parseMarkdownBlocks(source) {
  const input=String(source??'').replace(/\r\n?/g,'\n');
  const lines=input.split('\n');
  const out=[];
  let i=0;
  while(i<lines.length && out.length<2000) {
    const line=lines[i];
    if (!line.trim()) {i++;continue;}
    const fence=line.match(FENCE);
    if (fence) {
      const opening=fence[1],lang=fence[2].slice(0,32);
      const marker=opening[0],num=opening.length;
      i++;const contents=[];
      const closing = new RegExp(`^ {0,3}${marker==='`'?'`':'~'}{${num},}\\s*$`);
      while(i<lines.length && !closing.test(lines[i])) {contents.push(lines[i]);i++;}
      if(i<lines.length)i++;
      out.push({type:'code',lang,value:contents.join('\n')});continue;
    }
    if (RULE.test(line)) {out.push({type:'hr'});i++;continue;}
    const h=line.match(HEADING);
    if (h) {out.push({type:'heading',level:h[1].length,inline:parseMarkdownInline(h[2])});i++;continue;}
    const quote=line.match(QUOTE);
    if (quote) {
      const content=[];
      while(i<lines.length) {const m=lines[i].match(QUOTE);if(!m)break;content.push(m[1]);i++;}
      out.push({type:'quote',children:parseMarkdownBlocks(content.join('\n'))});continue;
    }
    const bullet=line.match(BULLET),numbered=line.match(ORDERED);
    if (bullet || numbered) {
      const ordered=Boolean(numbered),items=[];
      const rx=ordered?ORDERED:BULLET;
      const start=ordered?Number(numbered[1]):undefined;
      while(i<lines.length) {const match=lines[i].match(rx);if(!match)break;items.push(parseMarkdownInline(match[ordered?2:1]));i++;}
      out.push({type:'list',ordered,start,items});continue;
    }
    if(i+1<lines.length && line.includes('|') && tableBreak(lines[i+1])) {
      const header=cells(line),alignment=cells(lines[i+1]);
      if(header.length===alignment.length && header.length<=12) {
        i+=2;const rows=[];
        while(i<lines.length && lines[i].includes('|') && rows.length<100) {
          const row=cells(lines[i]);
          if(row.length!==header.length)break;
          rows.push(row.map(v=>parseMarkdownInline(v)));i++;
        }
        out.push({type:'table',header:header.map(v=>parseMarkdownInline(v)),rows});continue;
      }
    }
    const paragraph=[];
    while(i<lines.length && lines[i].trim()) {
      if(paragraph.length && (special(lines[i]) || (i+1<lines.length && lines[i].includes('|') && tableBreak(lines[i+1])))) break;
      paragraph.push(lines[i].trim());i++;
    }
    out.push({type:'paragraph',inline:parseMarkdownInline(paragraph.join('\n'))});
  }
  return out;
}
