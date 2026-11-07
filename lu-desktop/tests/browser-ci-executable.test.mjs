// Execute a headed browser from the assembled candidate, not the GitHub
// runner's globally installed executable. Linux must use Xvfb.
// This remains a CI acceptance fixture, not a CAPTCHA bypass.
import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync,readFileSync} from 'node:fs';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const bundle=resolve(import.meta.dirname,'../../out/lu-browser-candidate');
const manifestPath=join(bundle,'manifest.json');
const candidateTest=existsSync(manifestPath)?test:test.skip;
const manifest=existsSync(manifestPath)?JSON.parse(readFileSync(manifestPath,'utf8')):null;
const browserPath=join(bundle,process.platform==='win32'?'chrome.exe':'chrome');
const nodePath=join(bundle,process.platform==='win32'?'node.exe':'node');
const workerPath=join(bundle,'lu_browser_worker.mjs');
const playwrightPath=join(bundle,'node_modules/playwright/index.mjs');
candidateTest('headed Chromium candidate starts and completes verification handoff in the same session',async()=>{
  assert.equal(manifest.status,'candidate');
  for(const p of [browserPath,nodePath,workerPath,playwrightPath])
    assert.ok(existsSync(p),p);
  const {chromium}=await import(pathToFileURL(playwrightPath).href);
  const {BrowserSession}=await import(pathToFileURL(workerPath).href);
  const profile=await mkdtemp(join(tmpdir(),'lu-ci-candidate-'));
  // Force the candidate's Chromium executable and JS packages. No fallback
  // to a browser pre-installed on the GitHub runner.
  let websocketInterceptions=0;
  const provider={
    launchPersistentContext:async(dir,options)=>{
      const context=await chromium.launchPersistentContext(
        dir,{...options,executablePath:browserPath}
      );
      // Wrap the genuine Playwright routing API, not a mock browser session.
      const original=context.routeWebSocket.bind(context);
      context.routeWebSocket=(pattern,handler)=>original(pattern,async socket=>{
        websocketInterceptions++;
        await handler(socket);
      });
      return context;
    }
  };
  const session=new BrowserSession({
    origins:['https://example.com'],profileDirectory:profile,
    chromiumProvider:provider,visibilityMode:'always-visible',
    softwareRendering:true
  });
  try{
    const initial=await session.start();
    assert.equal(initial.status,'READY');
    assert.equal(initial.windowVisibility,'VISIBLE');
    // Route fixtures are entirely synthetic and do not depend on the public
    // site, provider anti-bot behavior or real user credentials.
    await session.context.route('https://example.com/lu-test',async route=>
      route.fulfill({status:200,contentType:'text/html',
        body:'<html><body><h1>Verify you are human</h1></body></html>'})
    );
    const gate=await session.execute({op:'navigate',url:'https://example.com/lu-test'});
    assert.equal(gate.status,'NEEDS_HUMAN');
    assert.equal(gate.reason,'CAPTCHA_REQUIRED');
    await assert.rejects(session.execute({op:'read'}),error=>error.code==='NEEDS_HUMAN');
    await session.page.setContent('<html><body>Real candidate browser resumed</body></html>');
    const resolved=await session.execute({op:'human_resolved'});
    assert.equal(resolved.status,'RESUMED');
    const text=await session.execute({op:'read'});
    assert.match(text.text,/Real candidate browser resumed/);
    // Real Chromium must hand off a login form with no "sign in" body text.
    await session.page.setContent('<html><body><input type="password"></body></html>');
    const login=await session.execute({op:'read'});
    assert.equal(login.status,'NEEDS_HUMAN');
    assert.equal(login.reason,'LOGIN_REQUIRED');
    await session.page.setContent('<html><body>Public archive fixture</body></html>');
    assert.equal((await session.execute({op:'human_resolved'})).status,'RESUMED');
    // The browser's HTTP request route does not cover WebSockets. Confirm
    // this actual Chromium page is intercepted and the socket is closed.
    const websocketResult=await session.page.evaluate(()=>new Promise(resolve=>{
      const socket=new WebSocket('wss://example.com/lu-test-denied');
      socket.onopen=()=>resolve('OPENED');
      socket.onclose=()=>resolve('CLOSED');
      setTimeout(()=>resolve('NOT_CLOSED'),4000);
    }));
    assert.equal(websocketResult,'CLOSED');
    assert.ok(websocketInterceptions>0);
    const screenshot=await session.execute({op:'screenshot'});
    assert.equal(screenshot.mime,'image/png');
    assert.ok(Buffer.from(screenshot.base64,'base64').length>500);
  }finally{
    await session.close();
    await rm(profile,{recursive:true,force:true});
  }
});
