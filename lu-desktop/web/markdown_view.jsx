import React from 'react';
import {parseMarkdownBlocks} from './markdown_view.mjs';

function Inline({nodes}) {
  return nodes.map((n,i) => {
    switch(n.type) {
      case 'text': return <React.Fragment key={i}>{n.value}</React.Fragment>;
      case 'code': return <code key={i}>{n.value}</code>;
      case 'strong': return <strong key={i}><Inline nodes={n.children}/></strong>;
      case 'em': return <em key={i}><Inline nodes={n.children}/></em>;
      case 'link': return <a key={i} href={n.href} target="_blank" rel="noreferrer noopener"><Inline nodes={n.children}/></a>;
      default: return null;
    }
  });
}
function Blocks({nodes}) {
  return nodes.map((n,i) => {
    switch(n.type) {
      case 'heading': return React.createElement(`h${n.level+1}`,{key:i},<Inline nodes={n.inline}/>);
      case 'paragraph':return <p key={i}><Inline nodes={n.inline}/></p>;
      case 'hr':return <hr key={i}/>;
      case 'quote':return <blockquote key={i}><Blocks nodes={n.children}/></blockquote>;
      case 'list': {
        const Tag=n.ordered?'ol':'ul';
        return <Tag key={i} start={n.ordered?n.start:undefined}>{n.items.map((item,j)=><li key={j}><Inline nodes={item}/></li>)}</Tag>;
      }
      case 'table':return <div className="markdownTableWrap" key={i}><table><thead><tr>{n.header.map((value,j)=><th key={j}><Inline nodes={value}/></th>)}</tr></thead><tbody>{n.rows.map((row,j)=><tr key={j}>{row.map((value,k)=><td key={k}><Inline nodes={value}/></td>)}</tr>)}</tbody></table></div>;
      case 'code':return <div className="codeBlock" key={i}><div className="codeBlockHeader"><span>{n.lang||'text'}</span><button type="button" aria-label="Copy code" onClick={() => navigator.clipboard?.writeText(n.value).catch(()=>{})}>Copy</button></div><pre><code>{n.value}</code></pre></div>;
      default:return null;
    }
  });
}
export function MarkdownView({text}) {
  return <div className="markdownBody"><Blocks nodes={parseMarkdownBlocks(text)}/></div>;
}
