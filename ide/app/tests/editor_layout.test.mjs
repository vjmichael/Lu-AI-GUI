import test from 'node:test';
import assert from 'node:assert/strict';
import {
 projectEditorTabs,secondaryEditor,initialSecondaryKey,canSaveFocusedPane
} from '../web/editor_layout.js';
const p='ide-'+'a'.repeat(32),q='ide-'+'b'.repeat(32);
const tabs=[
 {projectId:p,key:p+':one.rs',relativePath:'one.rs',dirty:true},
 {projectId:p,key:p+':two.rs',relativePath:'two.rs',dirty:false},
 {projectId:q,key:q+':secret.rs',relativePath:'secret.rs',dirty:true},
];
test('two real CodeMirror panes require different already-open tabs in same registered project',()=>{
 assert.equal(projectEditorTabs(tabs,p).length,2);
 assert.equal(initialSecondaryKey(tabs,p,tabs[0].key),tabs[1].key);
 assert.equal(secondaryEditor(tabs,p,tabs[0].key,tabs[1].key),tabs[1]);
 assert.equal(secondaryEditor(tabs,p,tabs[0].key,tabs[0].key),null);
 assert.equal(secondaryEditor(tabs,p,tabs[0].key,tabs[2].key),null);
 assert.equal(secondaryEditor(tabs,q,tabs[0].key,tabs[2].key),tabs[2]);
 assert.equal(secondaryEditor(tabs,p,tabs[0].key,'invented'),null);
});
test('file closure, project switch and absence of second file fail closed',()=>{
 assert.equal(secondaryEditor(tabs.filter(x=>x.key!==tabs[1].key),p,tabs[0].key,tabs[1].key),null);
 assert.equal(initialSecondaryKey(tabs.filter(x=>x.projectId===q),q,tabs[2].key),null);
 assert.equal(projectEditorTabs(tabs,null).length,0);
 assert.equal(secondaryEditor([],p,tabs[0].key,tabs[1].key),null);
});
test('Ctrl+S targets the actually focused pane and never silently falls through',()=>{
 assert.equal(canSaveFocusedPane('main',tabs[0],tabs[1]).key,tabs[0].key);
 assert.equal(canSaveFocusedPane('secondary',tabs[0],tabs[1]).key,tabs[1].key);
 assert.equal(canSaveFocusedPane('secondary',tabs[0],null),null);
});
