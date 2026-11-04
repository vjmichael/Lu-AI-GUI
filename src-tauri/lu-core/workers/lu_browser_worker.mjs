// TOOLS-10B: first-party browser worker protocol. No TCP listener, no shell,
// no model-supplied JavaScript execution or CAPTCHA solving.
// The trusted Rust host must authenticate/authorize every incoming command.
import { pathToFileURL } from 'node:url';
import readline from 'node:readline';

export class BrowserGateError extends Error {
  constructor(code) { super(code); this.code = code; }
}
export function approvedUrl(value, origins) {
  let url;
  try { url = new URL(value); } catch { throw new BrowserGateError('INVALID_URL'); }
  if (url.protocol !== 'https:' || url.username || url.password || url.port && url.port !== '443') {
    throw new BrowserGateError('INVALID_URL');
  }
  if (!origins.has(url.origin)) throw new BrowserGateError('ORIGIN_NOT_GRANTED');
  return url;
}
export function challengeKind(text) {
  const snippet = String(text).slice(0, 120_000).toLowerCase();
  if (/verify (that )?you are human|complete the captcha|recaptcha|hcaptcha|unusual traffic|checking your browser/.test(snippet)) {
    return 'CAPTCHA_REQUIRED';
  }
  if (/sign in to continue|log in to continue|please sign in|authentication required/.test(snippet)) {
    return 'LOGIN_REQUIRED';
  }
  return null;
}
function boundedText(value, max) {
  if (typeof value !== 'string' || !value.length || value.length > max || /[\x00-\x08\x0b-\x1f]/.test(value)) {
    throw new BrowserGateError('INVALID_INPUT');
  }
  return value;
}
// Native host acknowledges window visibility through a dedicated reply type.
export class HostVisibilityController {
  constructor({write,timeoutMs=5000}={}) {
    if(typeof write!=='function'||!Number.isInteger(timeoutMs)||timeoutMs<1||timeoutMs>30000)
      throw new BrowserGateError('INVALID_HOST_BRIDGE');
    this.write=write;this.timeoutMs=timeoutMs;this.sequence=0;this.pending=new Map();
  }
  setVisible({visible}) {
    if(typeof visible!=='boolean') throw new BrowserGateError('INVALID_VISIBILITY');
    const requestId='visibility-'+(++this.sequence);
    return new Promise(resolve=>{
      const timer=setTimeout(()=>{this.pending.delete(requestId);resolve(false);},this.timeoutMs);
      this.pending.set(requestId,{timer,resolve});
      try {this.write(JSON.stringify({type:'host_visibility_request',requestId,visible})+'\n');}
      catch {clearTimeout(timer);this.pending.delete(requestId);resolve(false);}
    });
  }
  acceptHostReply(message) {
    if(!message||message.type!=='host_visibility_response'||
       typeof message.requestId!=='string'||typeof message.ok!=='boolean')return false;
    const target=this.pending.get(message.requestId);
    if(!target)return false;
    clearTimeout(target.timer);this.pending.delete(message.requestId);
    target.resolve(message.ok);return true;
  }
  close() {
    for(const item of this.pending.values()){clearTimeout(item.timer);item.resolve(false);}
    this.pending.clear();
  }
}

export class BrowserSession {
  constructor({origins, profileDirectory, chromiumProvider, softwareRendering = false,
    visibilityMode = 'automatic', windowController = null, cdpEndpoint = null} = {}) {
    if (!Array.isArray(origins) || origins.length === 0 || origins.length > 32) {
      throw new BrowserGateError('INVALID_ORIGINS');
    }
    this.origins = new Set(origins.map(o => {
      const parsed = new URL(o);
      if (parsed.protocol !== 'https:' || parsed.origin !== o) throw new BrowserGateError('INVALID_ORIGINS');
      return o;
    }));
    if (typeof profileDirectory !== 'string' || !profileDirectory.length) {
      throw new BrowserGateError('PROFILE_NOT_SELECTED');
    }
    this.profileDirectory = profileDirectory; // host owns path and access control
    this.chromiumProvider = chromiumProvider;
    this.softwareRendering = softwareRendering === true;
    if (!['automatic','always-visible'].includes(visibilityMode)) {
      throw new BrowserGateError('INVALID_VISIBILITY_MODE');
    }
    this.visibilityMode = visibilityMode;
    // Only a host-owned native window controller may manipulate Chromium's window.
    // Playwright Page/Browser APIs must not pretend a window was hidden.
    this.windowController = windowController;
    if(cdpEndpoint !== null && !/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(cdpEndpoint)) {
      throw new BrowserGateError('UNTRUSTED_CDP_ENDPOINT');
    }
    this.cdpEndpoint=cdpEndpoint;
    this.connectedBrowser=null;
    this.windowVisibility = 'NOT_STARTED';
    this.context = null;
    this.page = null;
    this.paused = null;
  }
  async start() {
    const provider = this.chromiumProvider ?? (await import('playwright').catch(() => {
      throw new BrowserGateError('BROWSER_BACKEND_UNAVAILABLE');
    })).chromium;
    if(this.cdpEndpoint !== null) {
      // Chromium was launched as a directly owned Child by the native Lu host.
      // Playwright attaches to that same headed process; it must not spawn
      // another browser whose window the host would not own.
      this.connectedBrowser=await provider.connectOverCDP(this.cdpEndpoint,{timeout:10000});
      this.context=this.connectedBrowser.contexts()[0];
      if(!this.context)throw new BrowserGateError('NO_OWNED_BROWSER_CONTEXT');
    } else {
      // Standalone/fixture mode. Automatic hiding requires a separately
      // authenticated native window controller; otherwise start fails closed.
      this.context = await provider.launchPersistentContext(this.profileDirectory, {
        headless: false, acceptDownloads: false, serviceWorkers: 'block',
        viewport: {width: 1280, height: 900},
        args: this.softwareRendering ? ['--disable-gpu'] : [],
      });
    }
    await this.context.route('**/*', async route => {
      try {
        approvedUrl(route.request().url(), this.origins);
        await route.continue();
      } catch { await route.abort(); }
    });
    this.page = this.context.pages()[0] ?? await this.context.newPage();
    try {
      if (this.visibilityMode === 'automatic') {
        await this.#setWindowVisibility('HIDDEN');
      } else {
        this.windowVisibility = 'VISIBLE';
      }
    } catch (error) {
      await this.close();
      throw error;
    }
    return {status:'READY',windowVisibility:this.windowVisibility};
  }
  async #setWindowVisibility(visibility) {
    if (!this.windowController || typeof this.windowController.setVisible !== 'function') {
      throw new BrowserGateError('NATIVE_WINDOW_CONTROLLER_UNAVAILABLE');
    }
    // A trusted controller must prove the window belongs to this same Chromium
    // session. It must not hide another application's window by title matching.
    const response = await this.windowController.setVisible({
      context:this.context,visible:visibility==='VISIBLE'
    });
    if (response !== true) throw new BrowserGateError('WINDOW_VISIBILITY_FAILED');
    this.windowVisibility=visibility;
  }
  async #requestHuman(reason) {
    this.paused=reason;
    // Never claim the browser was revealed unless the native window API
    // confirms success. If unavailable, the challenge remains paused.
    if (this.visibilityMode==='automatic') await this.#setWindowVisibility('VISIBLE');
    return {status:'NEEDS_HUMAN',reason,url:this.page.url(),
      windowVisibility:this.windowVisibility};
  }
  async execute(command) {
    if (!this.page) throw new BrowserGateError('BROWSER_NOT_STARTED');
    if (!command || typeof command !== 'object' || Array.isArray(command)) {
      throw new BrowserGateError('INVALID_COMMAND');
    }
    const op = command.op;
    if (op === 'status') {
      return {status:this.paused ? 'NEEDS_HUMAN' : 'READY',reason:this.paused ?? null,
        url:this.page.url(),windowVisibility:this.windowVisibility,visibilityMode:this.visibilityMode};
    }
    if (op === 'human_resolved') {
      if (!this.paused) throw new BrowserGateError('NO_PENDING_CHALLENGE');
      // The user interacts with the existing visible browser. Re-check current
      // page instead of blindly assuming the challenge has been completed.
      approvedUrl(this.page.url(),this.origins);
      const text = await this.page.locator('body').innerText({timeout:5000});
      if (challengeKind(text)) throw new BrowserGateError('CHALLENGE_STILL_PRESENT');
      // User may have completed a form or navigated. Verify challenge state
      // before the host can hide the window and relinquish human control.
      if (this.visibilityMode==='automatic') {
        // Do not release human ownership until the native host confirms
        // the window is hidden. A failed hide remains a visible takeover.
        await this.#setWindowVisibility('HIDDEN');
      }
      this.paused = null;
      return {status:'RESUMED',url:this.page.url(),windowVisibility:this.windowVisibility};
    }
    if (this.paused) throw new BrowserGateError('NEEDS_HUMAN');
    if (op === 'show_browser') {
      await this.#setWindowVisibility('VISIBLE');
      return {status:'VISIBLE',windowVisibility:this.windowVisibility};
    }
    if (op === 'hide_browser') {
      if (this.visibilityMode==='always-visible') throw new BrowserGateError('VISIBILITY_LOCKED_VISIBLE');
      await this.#setWindowVisibility('HIDDEN');
      return {status:'HIDDEN',windowVisibility:this.windowVisibility};
    }
    if (op === 'navigate') {
      const url = approvedUrl(boundedText(command.url,2048),this.origins);
      const response = await this.page.goto(url.href,{waitUntil:'domcontentloaded',timeout:20000});
      approvedUrl(this.page.url(),this.origins);
      const text = await this.page.locator('body').innerText({timeout:5000}).catch(()=>'');
      const challenge = challengeKind(text);
      if (challenge) {
        return this.#requestHuman(challenge);
      }
      return {status:'NAVIGATED',url:this.page.url(),httpStatus:response?.status()??null};
    }
    if (op === 'click') {
      const x=command.x,y=command.y;
      if (!Number.isInteger(x) || !Number.isInteger(y) || x<0 || x>1280 || y<0 || y>900) {
        throw new BrowserGateError('INVALID_COORDINATES');
      }
      await this.page.mouse.click(x,y);
      return {status:'CLICKED'};
    }
    if (op === 'type') {
      const text=boundedText(command.text,2048);
      await this.page.keyboard.type(text,{delay:15});
      return {status:'TYPED',length:text.length};
    }
    if (op === 'press') {
      const key=boundedText(command.key,32);
      const allowed=new Set(['Enter','Tab','Escape','Backspace','ArrowUp','ArrowDown',
        'ArrowLeft','ArrowRight','PageUp','PageDown','Home','End']);
      if (!allowed.has(key)) throw new BrowserGateError('UNAPPROVED_KEY');
      await this.page.keyboard.press(key);
      return {status:'KEY_PRESSED',key};
    }
    if (op === 'click_selector') {
      const selector=boundedText(command.selector,256);
      if (/javascript:|:has\(|:visited|\[href\^=/.test(selector)) {
        throw new BrowserGateError('INVALID_SELECTOR');
      }
      await this.page.locator(selector).first().click({timeout:5000});
      return {status:'CLICKED'};
    }
    if (op === 'back' || op === 'forward') {
      if (op === 'back') await this.page.goBack({waitUntil:'domcontentloaded',timeout:15000});
      else await this.page.goForward({waitUntil:'domcontentloaded',timeout:15000});
      approvedUrl(this.page.url(),this.origins);
      return {status:'NAVIGATED',url:this.page.url()};
    }
    if (op === 'accessibility') {
      const tree=await this.page.locator('body').ariaSnapshot({timeout:5000});
      return {status:'ACCESSIBILITY',url:this.page.url(),text:tree.slice(0,65536),
        truncated:tree.length>65536};
    }
    if (op === 'scroll') {
      const delta=command.delta;
      if (!Number.isInteger(delta) || Math.abs(delta)>3000) throw new BrowserGateError('INVALID_SCROLL');
      await this.page.mouse.wheel(0,delta);
      return {status:'SCROLLED'};
    }
    if (op === 'read') {
      const text = await this.page.locator('body').innerText({timeout:5000});
      const challenge=challengeKind(text);
      if (challenge) {
        return this.#requestHuman(challenge);
      }
      return {status:'READ',url:this.page.url(),text:text.slice(0,131072),truncated:text.length>131072};
    }
    if (op === 'screenshot') {
      let image;
      try {
        await this.page.bringToFront();
        image=await this.page.screenshot({type:'png',animations:'disabled',timeout:10000});
      } catch {
        // Some Linux Xvfb GPU stacks need a second frame after focus.
        await this.page.waitForTimeout(200);
        try {
          image=await this.page.screenshot({type:'png',animations:'allow',timeout:10000});
        } catch { throw new BrowserGateError('SCREENSHOT_UNAVAILABLE'); }
      }
      if (image.length>2*1024*1024) throw new BrowserGateError('SCREENSHOT_TOO_LARGE');
      return {status:'SCREENSHOT',mime:'image/png',base64:image.toString('base64')};
    }
    throw new BrowserGateError('UNKNOWN_OPERATION');
  }
  async close() {
    if (this.connectedBrowser) {
      // CDP detach is not a native process shutdown. Rust owns termination.
      await this.connectedBrowser.close();
      this.connectedBrowser=null;
    } else if (this.context) {
      await this.context.close();
    }
    this.context=null;this.page=null;this.windowVisibility='NOT_STARTED';
  }
}
async function main() {
  const config=JSON.parse(process.env.LU_BROWSER_HOST_CONFIG || '{}');
  const bridge=new HostVisibilityController({write:line=>process.stdout.write(line)});
  const session=new BrowserSession({...config,windowController:bridge});
  let started=false;
  let queue=Promise.resolve();
  const input=readline.createInterface({input:process.stdin,crlfDelay:Infinity});
  input.on('line',line=>{
    let request;
    try {
      if(line.length>8192)throw new BrowserGateError('COMMAND_TOO_LARGE');
      request=JSON.parse(line);
    } catch {
      process.stdout.write(JSON.stringify({ok:false,error:{code:'INVALID_COMMAND'}})+'\n');
      return;
    }
    if(request?.type==='host_visibility_response'){
      bridge.acceptHostReply(request);
      return;
    }
    // Commands are serialized; host replies are not. Otherwise first
    // automatic hide deadlocks waiting for its own acknowledgement.
    queue=queue.then(async()=>{
      try {
        if(!started){await session.start();started=true;}
        const result=await session.execute(request);
        process.stdout.write(JSON.stringify({ok:true,result})+'\n');
      }catch(error){
        const code=error instanceof BrowserGateError?error.code:'BROWSER_FAILED';
        process.stdout.write(JSON.stringify({ok:false,error:{code}})+'\n');
      }
    });
  });
  await new Promise(resolve=>input.once('close',resolve));
  bridge.close();
  await queue;
  await session.close();
}
if (process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  main().catch(() => {process.exitCode=1;});
}
