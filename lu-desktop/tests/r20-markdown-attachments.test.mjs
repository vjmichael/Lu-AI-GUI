import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {parseMarkdownBlocks,parseMarkdownInline,safeMarkdownHref} from '../web/markdown_view.mjs';
import {buildSubmittedPrompt,cleanAttachmentName,attachmentSummary,MAX_TEXT_FILES} from '../web/attachment_drafts.mjs';
const ui=readFileSync(new URL('../web/main.jsx',import.meta.url),'utf8');
const jsx=readFileSync(new URL('../web/markdown_view.jsx',import.meta.url),'utf8');
const css=readFileSync(new URL('../web/styles.css',import.meta.url),'utf8');

test('headings, lists, paragraphs and fenced code become distinct AST nodes',()=>{
  const result=parseMarkdownBlocks('## Head\n\n1. First\n2. Second\n\n```python\nprint(1)\n```\n\nLast **bold** text');
  assert.deepEqual(result.map(x=>x.type),['heading','list','code','paragraph']);
  assert.equal(result[1].ordered,true);
  assert.equal(result[2].lang,'python');
  assert.equal(result[2].value,'print(1)');
  assert.equal(result[3].inline[1].type,'strong');
});
test('incomplete streamed Markdown is still shown without throwing',()=>{
  const p=parseMarkdownBlocks('Before\n\n```rust\nfn main() {}');
  assert.deepEqual(p.map(x=>x.type),['paragraph','code']);
  assert.equal(p[1].value,'fn main() {}');
  assert.equal(parseMarkdownInline('Unfinished **bold').map(x=>x.value).join(''),'Unfinished **bold');
});
test('HTML is inert text and unsafe URI schemes never generate a link node',()=>{
  const p=parseMarkdownBlocks('<img src=x onerror=alert(1)>\n\n[bad](javascript:alert%281%29)');
  assert.equal(p[0].inline[0].type,'text');
  assert.equal(p[0].inline[0].value,'<img src=x onerror=alert(1)>');
  assert.equal(safeMarkdownHref('javascript:alert(1)'),null);
  assert.equal(safeMarkdownHref('file:///C:/Users/secrets'),null);
  assert.equal(safeMarkdownHref('data:text/html;base64,YQ=='),null);
  assert.equal(safeMarkdownHref('https://u:pass@example.com'),null);
  assert.equal(safeMarkdownHref('https://example.com/'), 'https://example.com/');
  assert.ok(!p[1].inline.some(x=>x.type==='link'));
  assert.ok(!jsx.includes('dangerouslySetInnerHTML'));
  assert.ok(!jsx.includes('innerHTML'));
});
test('simple pipe table and blockquote render as typed nodes',()=>{
  const p=parseMarkdownBlocks('| Name | Value |\n| --- | --- |\n| A | 1 |\n\n> Quote');
  assert.deepEqual(p.map(x=>x.type),['table','quote']);
  assert.equal(p[0].rows.length,1);
  assert.equal(p[0].rows[0][1][0].value,'1');
});
test('code blocks expose copy through trusted click, not HTML execution',()=>{
  assert.ok(jsx.includes('navigator.clipboard?.writeText(n.value)'));
  assert.ok(jsx.includes('rel="noreferrer noopener"'));
  assert.ok(css.includes('.markdownBody .codeBlock'));
  assert.ok(css.includes('.markdownTableWrap'));
});
test('typed text drafts remain separate from textarea and are size-bound on send',()=>{
  const file={name:'my notes.md',size:1024,text:'two lines\nnext line'};
  assert.equal(buildSubmittedPrompt('Summarize', [file]),'Summarize\n\n[Attached text file: my notes.md]\ntwo lines\nnext line\n[/Attached text file]');
  assert.throws(()=>buildSubmittedPrompt(' ',[file]),/Type a question/);
  assert.throws(()=>buildSubmittedPrompt('Prompt',[{...file,text:'X'.repeat(16400)}]),/limit/);
  assert.equal(cleanAttachmentName('../a[b].txt'),'.._a_b_.txt');
  assert.equal(attachmentSummary(file),'my notes.md · 1 KB');
  assert.equal(MAX_TEXT_FILES,4);
});
test('draft attachments survive switching conversations and have remove controls',()=>{
  assert.ok(ui.includes('draftFiles.current = draft.files || []'));
  assert.ok(ui.includes('files: draftFiles.current'));
  assert.ok(ui.includes('setTextFiles(draft.files || [])'));
  assert.ok(ui.includes('aria-label="Draft attachments"'));
  assert.ok(ui.includes('URL.revokeObjectURL'));
  assert.ok(ui.includes('setImages([]); setTextFiles([]);'));
  assert.ok(ui.includes('buildSubmittedPrompt(input, textFiles)'));
});
test('image thumbnails work under image-only blob CSP without scripting changes',()=>{
  const conf=JSON.parse(readFileSync(new URL('../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
  const csp=conf.app.security.csp;
  assert.ok(csp.includes("img-src 'self' data: blob:"));
  assert.ok(!csp.includes('unsafe-eval'));
  assert.ok(ui.includes('URL.createObjectURL(file)'));
});
test('image and text attachment completion is scoped to the originating chat',()=>{
  assert.ok(ui.includes("const targetChat = currentChat.current || 'new';"));
  assert.ok(ui.includes('storeAttachment(\'files\',candidate)'));
  assert.ok(ui.includes("storeAttachment('images',[...previous"));
  assert.ok(ui.includes('if (key === (currentChat.current||\'new\'))'));
});
test('user authored think tags remain literal user text, not reasoning blocks',()=>{
  assert.ok(ui.includes("if (role === 'user') return <div className=\"messageContent userText\">"));
  assert.ok(ui.includes('role={entry.role}'));
});
