import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowserSession, BrowserGateError, HostVisibilityController, approvedUrl, challengeKind } from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

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
test('browser worker accepts two approved research sites but never expands grant', async () => {
  const approved=['https://archive.example','https://catalog.example'];
  let current='about:blank',routeHandler;
  const page={
    url:()=>current,
    goto:async url=>{current=url;return {status:()=>200};},
    locator:()=>({innerText:async()=> 'Verified page content'}),
  };
  const context={
    routeWebSocket:async()=>{},route:async (_pattern,handler)=>{routeHandler=handler;},
    pages:()=>[page],close:async()=>{},
  };
  const worker=new BrowserSession({origins:approved,
    profileDirectory:'/host/approved/profile',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:nativeWindow()});
  await worker.start();
  for(const url of ['https://archive.example/a','https://catalog.example/b']){
    assert.equal((await worker.execute({op:'navigate',url})).status,'NAVIGATED');
    const read=await worker.execute({op:'read'});
    assert.equal(read.url,url);
    assert.equal(read.status,'READ');
  }
  await assert.rejects(worker.execute({op:'navigate',url:'https://outsider.example/c'}),
    e=>e.code==='ORIGIN_NOT_GRANTED');
  let aborted=false;
  await routeHandler({request:()=>({url:'https://outsider.example/resource'}),
    continue:async()=>{},abort:async()=>{aborted=true;}});
  assert.equal(aborted,true);
  await worker.close();
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
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
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
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
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
    locator:()=>({innerText:async()=> 'A source',ariaSnapshot:async()=>'- heading "A source"'}),
  };
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
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
    routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}
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
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[],newPage:async()=>({}),close:async()=>{closed=true}};
  const session=new BrowserSession({origins:['https://archive.example'],
    profileDirectory:'/host/chosen/profile',
    chromiumProvider:{launchPersistentContext:async()=>context}});
  await assert.rejects(session.start(),e=>e.code==='NATIVE_WINDOW_CONTROLLER_UNAVAILABLE');
  assert.equal(closed,true);
});
test('always-visible setting never silently hides window', async () => {
  const page={url:()=> 'https://archive.example'};
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
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
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
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

test('visibility handshake is distinct from browser commands and requires native acknowledgement', async () => {
  const frames=[];
  const bridge=new HostVisibilityController({write:line=>frames.push(JSON.parse(line)),timeoutMs:100});
  const hide=bridge.setVisible({visible:false});
  assert.equal(frames.length,1);
  assert.equal(frames[0].type,'host_visibility_request');
  assert.equal(frames[0].visible,false);
  assert.equal(bridge.acceptHostReply({type:'host_visibility_response',
    requestId:'unknown',ok:true}),false);
  assert.equal(bridge.acceptHostReply({type:'host_visibility_response',
    requestId:frames[0].requestId,ok:true}),true);
  assert.equal(await hide,true);
  assert.equal(bridge.acceptHostReply({type:'host_visibility_response',
    requestId:frames[0].requestId,ok:true}),false);
  const show=bridge.setVisible({visible:true});
  assert.equal(frames[1].visible,true);
  bridge.close();
  assert.equal(await show,false);
});
test('lost native visibility acknowledgement times out closed', async () => {
  const bridge=new HostVisibilityController({write:()=>{},timeoutMs:5});
  assert.equal(await bridge.setVisible({visible:false}),false);
  bridge.close();
});

test('trusted loopback CDP attaches to one native-owned headed browser without relaunch',async()=>{
  let connected=0,launched=0;
  const page={url:()=> 'https://archive.example/record'};
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page]};
  const controller=nativeWindow();
  const provider={
    connectOverCDP:async endpoint=>{
      assert.equal(endpoint,'http://127.0.0.1:41007');
      connected++;
      return {contexts:()=>[context],close:async()=>{}};
    },
    launchPersistentContext:async()=>{launched++;throw Error('Must not relaunch');}
  };
  const worker=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/owned/profile',
    cdpEndpoint:'http://127.0.0.1:41007',chromiumProvider:provider,
    windowController:controller
  });
  const ready=await worker.start();
  assert.equal(ready.windowVisibility,'HIDDEN');
  assert.equal(connected,1);
  assert.equal(launched,0);
  assert.deepEqual(controller.calls.map(x=>x.visible),[false]);
  await worker.close();
});
test('rejects non-loopback CDP targets and remote-debugging websocket injections',()=>{
  for(const url of ['http://192.168.1.4:9222','https://127.0.0.1:9222',
    'http://localhost:9222','http://127.0.0.1:9222/path',
    'http://127.0.0.1:9222@evil.example']) {
    assert.throws(()=>new BrowserSession({
      origins:['https://archive.example'],profileDirectory:'/trusted/profile',
      cdpEndpoint:url
    }),e=>e.code==='UNTRUSTED_CDP_ENDPOINT');
  }
});

test('browser mouse and keyboard cannot act on a page outside granted origin',async()=>{
  let location='https://archive.example/record';
  let clicks=0,types=0;
  const page={
    url:()=>location,
    mouse:{click:async()=>{clicks++;}},
    keyboard:{type:async()=>{types++;}}
  };
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/profile',
    visibilityMode:'always-visible',
    chromiumProvider:{launchPersistentContext:async()=>context}
  });
  await session.start();
  location='https://unapproved.example/redirect';
  await assert.rejects(session.execute({op:'click',x:1,y:1}),
    e=>e.code==='ORIGIN_NOT_GRANTED');
  await assert.rejects(session.execute({op:'type',text:'sensitive'}),
    e=>e.code==='ORIGIN_NOT_GRANTED');
  assert.equal(clicks,0);
  assert.equal(types,0);
  await session.close();
});

test('history traversal pauses and reveals an encountered human challenge', async()=>{
  let current='https://archive.example/article',body='Article text';
  const visible=[];
  const page={
    url:()=>current,
    goBack:async()=>{current='https://archive.example/sign-in';body='Sign in to continue';},
    goForward:async()=>{current='https://archive.example/article';body='Article text';},
    locator:()=>({innerText:async()=>body})
  };
  const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/profile',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:{setVisible:async({visible:v})=>{visible.push(v);return true;}}
  });
  await session.start();
  const challenge=await session.execute({op:'back'});
  assert.equal(challenge.status,'NEEDS_HUMAN');
  assert.equal(challenge.reason,'LOGIN_REQUIRED');
  assert.deepEqual(visible,[false,true]);
  await assert.rejects(session.execute({op:'forward'}),e=>e.code==='NEEDS_HUMAN');
  body='Successfully signed in';
  const resumed=await session.execute({op:'human_resolved'});
  assert.equal(resumed.status,'RESUMED');
  assert.deepEqual(visible,[false,true,false]);
  await session.close();
});

test('origin-firewall initialization failure closes owned browser context',async()=>{
  let closed=false;
  const context={
    routeWebSocket:async()=>{},route:async()=>{throw new Error('route unavailable');},
    pages:()=>[],close:async()=>{closed=true;}
  };
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/profile',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:nativeWindow()
  });
  await assert.rejects(session.start(),/route unavailable/);
  assert.equal(closed,true);
});

test('accessibility and screenshot respect login challenge before reading pixels or tree',async()=>{
  for(const op of ['accessibility','screenshot']){
    let screenshotCalls=0,treeCalls=0;
    const page={
      url:()=> 'https://archive.example/login',
      locator:()=>({
        innerText:async()=> 'Sign in to continue',
        ariaSnapshot:async()=>{treeCalls++;return 'private';}
      }),
      screenshot:async()=>{screenshotCalls++;return Buffer.from('pixels');}
    };
    const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
    const controller=nativeWindow();
    const session=new BrowserSession({
      origins:['https://archive.example'],profileDirectory:'/host/profile',
      chromiumProvider:{launchPersistentContext:async()=>context},
      windowController:controller
    });
    await session.start();
    const result=await session.execute({op});
    assert.equal(result.status,'NEEDS_HUMAN');
    assert.equal(result.reason,'LOGIN_REQUIRED');
    assert.equal(treeCalls,0);
    assert.equal(screenshotCalls,0);
    assert.deepEqual(controller.calls.map(call=>call.visible),[false,true]);
    await session.close();
  }
});

test('failed challenge inspection never permits screenshot or accessibility output',async()=>{
  for(const op of ['screenshot','accessibility']){
    let accessed=false;
    const page={
      url:()=> 'https://archive.example/item',
      locator:()=>({
        innerText:async()=>{throw new Error('renderer unavailable');},
        ariaSnapshot:async()=>{accessed=true;return 'secret';}
      }),
      bringToFront:async()=>{accessed=true;},
      screenshot:async()=>{accessed=true;return Buffer.from('secret');}
    };
    const context={routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}};
    const session=new BrowserSession({
      origins:['https://archive.example'],profileDirectory:'/host/profile',
      chromiumProvider:{launchPersistentContext:async()=>context},
      windowController:nativeWindow()
    });
    await session.start();
    await assert.rejects(session.execute({op}),e=>e.code==='CHALLENGE_STATE_UNAVAILABLE');
    assert.equal(accessed,false);
    await session.close();
  }
});


test('visible password form triggers human handoff even with neutral page text',async()=>{
  let visible=true;
  const page={
    url:()=> 'https://archive.example/account',
    locator:selector=>selector==='body'
      ?{innerText:async()=> 'Welcome to the archive'}
      :{first:()=>({isVisible:async()=>selector==='input[type="password"]'&&visible})}
  };
  const native=nativeWindow();
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/private',
    chromiumProvider:{launchPersistentContext:async()=>({
      routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}
    })},
    windowController:native
  });
  await session.start();
  const gated=await session.execute({op:'read'});
  assert.equal(gated.status,'NEEDS_HUMAN');
  assert.equal(gated.reason,'LOGIN_REQUIRED');
  assert.deepEqual(native.calls.map(x=>x.visible),[false,true]);
  await assert.rejects(session.execute({op:'accessibility'}),e=>e.code==='NEEDS_HUMAN');
  await assert.rejects(session.execute({op:'human_resolved'}),
    e=>e.code==='CHALLENGE_STILL_PRESENT');
  visible=false;
  assert.equal((await session.execute({op:'human_resolved'})).status,'RESUMED');
  assert.equal((await session.execute({op:'read'})).status,'READ');
  await session.close();
});
test('visible embedded CAPTCHA is not missed by plain body-text detection',async()=>{
  let challengeVisible=true;
  const page={
    url:()=> 'https://archive.example/article',
    locator:selector=>selector==='body'
      ?{innerText:async()=> 'Article title and abstract'}
      :{first:()=>({isVisible:async()=>selector.includes('recaptcha/api2/bframe')&&challengeVisible})}
  };
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/private',
    chromiumProvider:{launchPersistentContext:async()=>({
      routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}
    })},
    windowController:nativeWindow()
  });
  await session.start();
  const gated=await session.execute({op:'read'});
  assert.equal(gated.status,'NEEDS_HUMAN');
  assert.equal(gated.reason,'CAPTCHA_REQUIRED');
  challengeVisible=false;
  assert.equal((await session.execute({op:'human_resolved'})).status,'RESUMED');
  await session.close();
});
test('unavailable challenge-state probe refuses to release page text',async()=>{
  const page={
    url:()=> 'https://archive.example/article',
    goto:async()=>({status:()=>200}),
    locator:selector=>selector==='body'
      ?{innerText:async()=> 'Potential private article text'}
      :{first:()=>({isVisible:async()=>{throw new Error('DOM detached')}})}
  };
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/private',
    chromiumProvider:{launchPersistentContext:async()=>({
      routeWebSocket:async()=>{},route:async()=>{},pages:()=>[page],close:async()=>{}
    })},
    windowController:nativeWindow()
  });
  await session.start();
  await assert.rejects(session.execute({op:'navigate',url:'https://archive.example/article'}),
    e=>e.code==='CHALLENGE_STATE_UNAVAILABLE');
  await assert.rejects(session.execute({op:'read'}),
    e=>e.code==='CHALLENGE_STATE_UNAVAILABLE');
  await session.close();
});


test('browser blocks page WebSockets without connecting to any server',async()=>{
  let routeSocket;
  const page={url:()=> 'https://archive.example/article'};
  const context={
    routeWebSocket:async (pattern,handler)=>{
      assert.equal(pattern,'**/*');
      routeSocket=handler;
    },
    route:async()=>{},pages:()=>[page],close:async()=>{}
  };
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/private',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:nativeWindow()
  });
  await session.start();
  let closed=false,connected=false;
  await routeSocket({
    close:async opts=>{closed=true;assert.equal(opts.code,1008);},
    connectToServer:()=>{connected=true;}
  });
  assert.equal(closed,true);
  assert.equal(connected,false);
  await session.close();
});
test('missing WebSocket interception fails before any page read',async()=>{
  let closed=false;
  const page={url:()=> 'https://archive.example/article'};
  const context={route:async()=>{},pages:()=>[page],close:async()=>{closed=true}};
  const session=new BrowserSession({
    origins:['https://archive.example'],profileDirectory:'/host/private',
    chromiumProvider:{launchPersistentContext:async()=>context},
    windowController:nativeWindow()
  });
  await assert.rejects(session.start(),e=>e.code==='WEBSOCKET_FIREWALL_UNAVAILABLE');
  assert.equal(closed,true);
});
