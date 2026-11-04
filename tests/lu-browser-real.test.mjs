import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {BrowserSession} from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

test('real visible Chromium worker starts, observes, screenshots and terminates', async () => {
  const profile=await mkdtemp(join(tmpdir(),'lu-chromium-ci-'));
  const browser=new BrowserSession({origins:['https://example.com'],profileDirectory:profile});
  try {
    const started=await browser.start();
    assert.equal(started.status,'READY');
    const status=await browser.execute({op:'status'});
    assert.equal(status.status,'READY');
    const screenshot=await browser.execute({op:'screenshot'});
    assert.equal(screenshot.mime,'image/png');
    assert.ok(Buffer.from(screenshot.base64,'base64').length>100);
  } finally {
    await browser.close();
    await rm(profile,{recursive:true,force:true});
  }
});
