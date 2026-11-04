import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowserSession, BrowserGateError, approvedUrl, challengeKind } from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

test('only approved HTTPS origins are navigable', () => {
  const allowed=new Set(['https://archive.example']);
  assert.equal(approvedUrl('https://archive.example/page',allowed).hostname,'archive.example');
  for (const url of ['http://archive.example','https://other.example','file:///etc/passwd',
                      'https://archive.example@private.local','https://archive.example:8443']) {
    assert.throws(()=>approvedUrl(url,allowed),BrowserGateError,url);
  }
});
test('CAPTCHA detection does not confuse normal pages with challenges', () => {
  assert.equal(challengeKind('Research paper and abstract'),null);
  assert.equal(challengeKind('Please complete the CAPTCHA'), 'CAPTCHA_REQUIRED');
  assert.equal(challengeKind('Sign in to continue'), 'LOGIN_REQUIRED');
});
test('a real browser session can be paused then user-resumed without auto-solving', async () => {
  let location='about:blank',text='A source abstract';
  const page={
    url:()=>location,
    goto:async url=>{location=url; text='Complete the CAPTCHA';return {status:()=>200};},
    locator:()=>({innerText:async()=>text}),
    mouse:{click:async()=>{},wheel:async()=>{}},
    keyboard:{type:async()=>{}},
    screenshot:async()=>Buffer.from('fake-screenshot')
  };
  const context={route:async()=>{},pages:()=>[page],close:async()=>{}};
  const chromiumProvider={launchPersistentContext:async()=>context};
  const session=new BrowserSession({origins:['https://archive.example'],
    profileDirectory:'/host/chosen/profile',chromiumProvider});
  await session.start();
  const challenge=await session.execute({op:'navigate',url:'https://archive.example/article'});
  assert.equal(challenge.status,'NEEDS_HUMAN');
  assert.equal(challenge.reason,'CAPTCHA_REQUIRED');
  await assert.rejects(session.execute({op:'click',x:5,y:5}),e=>e.code==='NEEDS_HUMAN');
  await assert.rejects(session.execute({op:'human_resolved'}),e=>e.code==='CHALLENGE_STILL_PRESENT');
  text='Article content verified by current browser session';
  const resumed=await session.execute({op:'human_resolved'});
  assert.equal(resumed.status,'RESUMED');
  assert.equal((await session.execute({op:'read'})).status,'READ');
  await session.close();
});
test('model-controlled script operations are refused', async () => {
  const page={url:()=> 'https://archive.example',locator:()=>({innerText:async()=> 'hello'})};
  const context={route:async()=>{},pages:()=>[page],close:async()=>{}};
  const session=new BrowserSession({origins:['https://archive.example'],
    profileDirectory:'/host/chosen/profile',chromiumProvider:{launchPersistentContext:async()=>context}});
  await session.start();
  await assert.rejects(session.execute({op:'evaluate',javascript:'alert(1)'}),e=>e.code==='UNKNOWN_OPERATION');
  await session.close();
});

test('bounded keyboard and accessibility operations require known actions', async () => {
  const pressed=[];
  const page={
    url:()=> 'https://archive.example',
    keyboard:{press:async key=>pressed.push(key)},
    locator:()=>({ariaSnapshot:async()=>'- heading "A source"'}),
  };
  const context={route:async()=>{},pages:()=>[page],close:async()=>{}};
  const session=new BrowserSession({origins:['https://archive.example'],
    profileDirectory:'/host/chosen/profile',
    chromiumProvider:{launchPersistentContext:async()=>context}});
  await session.start();
  assert.equal((await session.execute({op:'press',key:'Enter'})).status,'KEY_PRESSED');
  assert.deepEqual(pressed,['Enter']);
  await assert.rejects(session.execute({op:'press',key:'Control+L'}),e=>e.code==='UNAPPROVED_KEY');
  assert.equal((await session.execute({op:'accessibility'})).status,'ACCESSIBILITY');
  await session.close();
});
