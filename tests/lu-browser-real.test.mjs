import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {BrowserSession} from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

test('real visible Chromium worker starts, observes, screenshots and terminates', async () => {
  const profile=await mkdtemp(join(tmpdir(),'lu-chromium-ci-'));
  const browser=new BrowserSession({origins:['https://example.com'],profileDirectory:profile,softwareRendering:true,visibilityMode:'always-visible'});
  try {
    const started=await browser.start();
    assert.equal(started.status,'READY');
    const status=await browser.execute({op:'status'});
    assert.equal(status.status,'READY');
    // Real Chromium processes the trusted synthetic HTML fixture without
    // relying on external sites or attempting any CAPTCHA circumvention.
    await browser.context.route('https://example.com/fixture',async route=>{
      await route.fulfill({status:200,contentType:'text/html',
        body:'<html><body><h1>Verify you are human</h1></body></html>'});
    });
    const first=await browser.execute({op:'navigate',url:'https://example.com/fixture'});
    assert.equal(first.status,'NEEDS_HUMAN');
    assert.equal(first.reason,'CAPTCHA_REQUIRED');
    await assert.rejects(browser.execute({op:'read'}),e=>e.code==='NEEDS_HUMAN');
    // Simulate the user legitimately completing an interactive challenge.
    await browser.page.setContent('<html><body><h1>Accessible archival entry</h1></body></html>');
    const resume=await browser.execute({op:'human_resolved'});
    assert.equal(resume.status,'RESUMED');
    const read=await browser.execute({op:'read'});
    assert.match(read.text,/Accessible archival entry/);
    const screenshot=await browser.execute({op:'screenshot'});
    assert.equal(screenshot.mime,'image/png');
    assert.ok(Buffer.from(screenshot.base64,'base64').length>100);
  } finally {
    await browser.close();
    await rm(profile,{recursive:true,force:true});
  }
});
