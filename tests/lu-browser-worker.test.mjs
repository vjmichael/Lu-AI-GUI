import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowserSession, BrowserGateError, approvedUrl, challengeKind } from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

const nativeWindow = () => ({
  calls:[],
  async setVisible({context,visible}) {
    this.calls.push({context,visible});
    return true;
  }
});

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
    profileDirectory:'/host/chosen/profile',chromiumProvider,windowController:nativeWindow()});
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
    profileDirectory:'/host/chosen/profile',chromiumProvider:{launchPersistentContext:async()=>context},windowController:nativeWindow()});
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

test('automatic mode starts headed-but-hidden, reveals same session for challenge, and re-hides on resume', async () => {
  let current='about:blank',body='Normal page';
  const context={
    route:async()=>{},pages:()=>[page],close:async()=>{}
  };
  const page={
    url:()=>current,
    goto:async url=>{current=url;body='Please complete the CAPTCHA';return {status:()=>200};},
    locator:()=>({innerText:async()=>body})
  };
  let launches=0,headless;
  const native=nativeWindow();
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/chosen/profile',
    chromiumProvider:{launchPersistentContext:async(_,opts)=>{
      launches++;headless=opts.headless;return context;
    }},
    windowController:native
  });
  const ready=await session.start();
  assert.equal(headless,false);
  assert.equal(ready.windowVisibility,'HIDDEN');
  assert.deepEqual(native.calls.map(x=>x.visible),[false]);
  const challenge=await session.execute({op:'navigate',url:'https://archive.example/item'});
  assert.equal(challenge.status,'NEEDS_HUMAN');
  assert.equal(challenge.windowVisibility,'VISIBLE');
  assert.deepEqual(native.calls.map(x=>x.visible),[false,true]);
  await assert.rejects(session.execute({op:'hide_browser'}),e=>e.code==='NEEDS_HUMAN');
  body='Full source content';
  const done=await session.execute({op:'human_resolved'});
  assert.equal(done.windowVisibility,'HIDDEN');
  assert.equal(launches,1);
  assert.deepEqual(native.calls.map(x=>x.visible),[false,true,false]);
  await session.close();
});
test('automatic mode fails closed without native window controller', async () => {
  let closed=false;
  const context={route:async()=>{},pages:()=>[],newPage:async()=>({}),close:async()=>{closed=true}};
  const session=new BrowserSession({origins:['https://archive.example'],
    profileDirectory:'/host/chosen/profile',
    chromiumProvider:{launchPersistentContext:async()=>context}});
  await assert.rejects(session.start(),e=>e.code==='NATIVE_WINDOW_CONTROLLER_UNAVAILABLE');
  assert.equal(closed,true);
});
test('always-visible setting never silently hides window', async () => {
  const page={url:()=> 'https://archive.example'};
  const context={route:async()=>{},pages:()=>[page],close:async()=>{}};
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/chosen/profile',
    visibilityMode:'always-visible',
    chromiumProvider:{launchPersistentContext:async()=>context}
  });
  const ready=await session.start();
  assert.equal(ready.windowVisibility,'VISIBLE');
  await assert.rejects(session.execute({op:'hide_browser'}),e=>e.code==='VISIBILITY_LOCKED_VISIBLE');
  await session.close();
});

test('failed re-hide keeps human takeover active and visible', async () => {
  let location='about:blank',body='Complete the CAPTCHA',hiddenCalls=0;
  const page={
    url:()=>location,
    goto:async url=>{location=url;return {status:()=>200};},
    locator:()=>({innerText:async()=>body})
  };
  const context={route:async()=>{},pages:()=>[page],close:async()=>{}};
  const controller={async setVisible({visible}) {
    if (!visible && ++hiddenCalls>1) return false;
    return true;
  }};
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/chosen/profile',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:controller
  });
  await session.start();
  await session.execute({op:'navigate',url:'https://archive.example/doc'});
  body='The human verification completed.';
  await assert.rejects(session.execute({op:'human_resolved'}),
    e=>e.code==='WINDOW_VISIBILITY_FAILED');
  const state=await session.execute({op:'status'});
  assert.equal(state.status,'NEEDS_HUMAN');
  assert.equal(state.windowVisibility,'VISIBLE');
  await session.close();
});
