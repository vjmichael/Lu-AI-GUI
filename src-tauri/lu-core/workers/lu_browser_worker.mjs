// TOOLS-10B: first-party browser worker protocol. No TCP listener, no shell,
// no model-supplied JavaScript execution or CAPTCHA solving.
// The trusted Rust host must authenticate/authorize every incoming command.
import { pathToFileURL } from 'node:url';
import readline from 'node:readline';
import { lookup } from 'node:dns/promises';
import { request as httpsRequest } from 'node:https';
import { isIPv4, isIP } from 'node:net';
import { createHash } from 'node:crypto';
import { basename } from 'node:path';

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
// CP-04B: public search is a deliberately narrower transport than general
// browsing. Only the exact host-approved Google query goes on the wire. No
// cookies, proxy, redirect follow, page-controlled networking or request body.
const SEARCH_ORIGIN='https://www.google.com';
const SEARCH_MAX_BYTES=2*1024*1024;
export function exactPublicSearchUrl(value) {
  const url=approvedUrl(value,new Set([SEARCH_ORIGIN]));
  const pairs=[...url.searchParams];
  if(url.pathname!=='/search'||url.hash||pairs.length!==1||pairs[0][0]!=='q'||
     !pairs[0][1].trim()||Buffer.byteLength(pairs[0][1])>512||/[\x00-\x1f\x7f]/.test(pairs[0][1]))
    throw new BrowserGateError('PUBLIC_SEARCH_INVALID_QUERY');
  return url;
}
export function isPublicSearchIpv4(address) {
  if(!isIPv4(address))return false;
  const [a,b,c]=address.split('.').map(Number);
  // IPv4-only, fail closed for special-use ranges (including CGNAT,
  // benchmarking, documentation, multicast, link-local and loopback).
  return a>0&&a<224&&a!==10&&a!==127&&!(a===100&&b>=64&&b<=127)&&
    !(a===169&&b===254)&&!(a===172&&b>=16&&b<=31)&&
    !(a===192&&(b===0||b===168||(b===88&&c===99)))&&
    !(a===198&&(b===18||b===19||(b===51&&c===100)))&&
    !(a===203&&b===0&&c===113);
}
export async function resolvePublicSearchAddress(resolve=lookup) {
  let timer;
  try {
    const records=await Promise.race([
      resolve('www.google.com',{family:4,all:true,verbatim:true}),
      new Promise((_,reject)=>{timer=setTimeout(()=>reject(new BrowserGateError('PUBLIC_SEARCH_OFFLINE')),5000);})
    ]);
    if(!Array.isArray(records)||!records.length||records.length>64||
      records.some(r=>r.family!==4||!isPublicSearchIpv4(r.address)))
      throw new BrowserGateError('PUBLIC_SEARCH_PRIVATE_ADDRESS');
    return records[0].address;
  } catch(e) {
    if(e instanceof BrowserGateError)throw e;
    throw new BrowserGateError('PUBLIC_SEARCH_OFFLINE');
  } finally {clearTimeout(timer);}
}
export async function fetchPinnedPublicSearch(value,{resolve=lookup,request=httpsRequest}={}) {
  const url=exactPublicSearchUrl(value);
  const address=await resolvePublicSearchAddress(resolve);
  return await new Promise((resolveResult,reject)=>{
    let req,timer,settled=false;
    const finish=(error,result)=>{if(settled)return;settled=true;clearTimeout(timer);if(error){req?.destroy();reject(error);}else resolveResult(result);};
    const fail=code=>finish(new BrowserGateError(code));
    timer=setTimeout(()=>fail('PUBLIC_SEARCH_OFFLINE'),12000);
    try {
      // This lookup returns precisely the checked address, not a second DNS
      // lookup. TLS authenticates the original www.google.com hostname/SNI.
      req=request({protocol:'https:',hostname:url.hostname,servername:url.hostname,
        port:443,path:url.pathname+url.search,method:'GET',agent:false,
        family:4,autoSelectFamily:false,rejectUnauthorized:true,
        lookup:(host,options,done)=>{
          if(host!==url.hostname)return done(new Error('unapproved host'));
          if(options?.all)return done(null,[{address,family:4}]);
          done(null,address,4);
        },headers:{'Accept':'text/html','Accept-Encoding':'identity',
          'User-Agent':'Mozilla/5.0 Lu public search (read-only)'},maxHeaderSize:16384},res=>{
          const status=res.statusCode;
          if(status===429){res.destroy();return fail('PUBLIC_SEARCH_CAPTCHA_REQUIRED');}
          if(status>=300&&status<400){res.destroy();return fail('PUBLIC_SEARCH_REDIRECT_BLOCKED');}
          if(status!==200){res.destroy();return fail('PUBLIC_SEARCH_HTTP_ERROR');}
          if(!/^text\/html(?:\s*;|$)/i.test(String(res.headers['content-type']||''))||
             (res.headers['content-encoding']&&res.headers['content-encoding']!=='identity')||
             Number(res.headers['content-length'])>SEARCH_MAX_BYTES){res.destroy();return fail('PUBLIC_SEARCH_INVALID_RESPONSE');}
          const chunks=[];let bytes=0;
          res.on('data',chunk=>{bytes+=chunk.length;if(bytes>SEARCH_MAX_BYTES){res.destroy();fail('PUBLIC_SEARCH_RESPONSE_TOO_LARGE');}else chunks.push(chunk);});
          res.on('error',()=>fail('PUBLIC_SEARCH_OFFLINE'));
          res.on('aborted',()=>fail('PUBLIC_SEARCH_OFFLINE'));
          res.on('end',()=>{
            if(!bytes||!res.complete)return fail('PUBLIC_SEARCH_INVALID_RESPONSE');
            const body=Buffer.concat(chunks);
            finish(null,{body,status,bodySha256:createHash('sha256').update(body).digest('hex')});
          });
        });
      req.on('error',()=>fail('PUBLIC_SEARCH_OFFLINE'));
      req.end();
    }catch{fail('PUBLIC_SEARCH_OFFLINE');}
  });
}
// Unwrap Google's displayed redirect links without visiting them. These are
// untrusted result labels, never approved navigation or independent citations.
export function displayedSearchTarget(href,base) {
  if(typeof href!=='string'||!href.length||href.length>8192)return null;
  try {
    let target=new URL(href,base);
    if(target.origin===SEARCH_ORIGIN&&target.pathname==='/url'){
      const values=[...target.searchParams].filter(([key])=>key==='q'||key==='url');
      if(values.length!==1)return null;
      target=new URL(values[0][1]);
    }
    if(target.protocol!=='https:'||target.username||target.password||target.port||
      target.hostname==='www.google.com'||target.href.length>2048||isIP(target.hostname.replace(/^\[|\]$/g,''))||
      !target.hostname.includes('.')||/(?:\.local|\.internal|\.localhost|\.home\.arpa)$/.test(target.hostname))return null;
    return target.href;
  }catch{return null;}
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
// This check is read-only: no challenge solving, page scripts, field values,
// or credentials are ever collected. A CAPTCHA iframe/login form need not
// announce itself in body.innerText(), so visible DOM markers are inspected
// before a model may read text, accessibility or pixels.
export async function inspectHumanGate(page, bodyText) {
  const fromText=challengeKind(bodyText);
  if(fromText==='CAPTCHA_REQUIRED') return fromText;
  const markers=[
    ['iframe[src*="/recaptcha/api2/bframe"]','CAPTCHA_REQUIRED'],
    ['iframe[src*="hcaptcha.com"]','CAPTCHA_REQUIRED'],
    ['iframe[src*="challenges.cloudflare.com"]','CAPTCHA_REQUIRED'],
    ['[data-sitekey].h-captcha','CAPTCHA_REQUIRED'],
    ['input[type="password"]','LOGIN_REQUIRED']
  ];
  for(const [selector,reason] of markers) {
    // Native Playwright locators implement first().isVisible(). Minimal
    // offline test doubles may only expose innerText and skip DOM probing.
    const locator=page.locator(selector);
    const first=typeof locator?.first==='function'?locator.first():locator;
    if(typeof first?.isVisible!=='function') continue;
    let visible;
    try {visible=await first.isVisible({timeout:1000});}
    catch {throw new BrowserGateError('CHALLENGE_STATE_UNAVAILABLE');}
    if(visible) return reason;
  }
  return fromText;
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
    visibilityMode = 'automatic', windowController = null, cdpEndpoint = null, publicSearchFetcher = fetchPinnedPublicSearch} = {}) {
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
    this.publicSearchOnly=basename(profileDirectory)==='lu-browser-public-search-profile';
    if(this.publicSearchOnly&&(this.origins.size!==1||!this.origins.has(SEARCH_ORIGIN)))
      throw new BrowserGateError('PUBLIC_SEARCH_PROVIDER_NOT_GRANTED');
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
    if(this.publicSearchOnly&&cdpEndpoint===null)throw new BrowserGateError('PUBLIC_SEARCH_ISOLATION_UNAVAILABLE');
    this.cdpEndpoint=cdpEndpoint;
    this.connectedBrowser=null;
    this.windowVisibility = 'NOT_STARTED';
    this.context = null;
    this.page = null;
    this.paused = null;
    this.publicSearchFetcher=publicSearchFetcher;
    this.publicSearchAttempted=false;
    this.publicSearchReceipt=null;
    this.publicSearchContext=null;
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
    try {
      // Ordinary HTTP interception does not cover page-origin WebSockets.
      // Until a separately approved NetworkWrite WebSocket policy exists,
      // block every page WebSocket before visiting any user-selected URL.
      // CDP belongs to the host's Node-side connection, not a page socket.
      if(typeof this.context.routeWebSocket!=='function')
        throw new BrowserGateError('WEBSOCKET_FIREWALL_UNAVAILABLE');
      await this.context.routeWebSocket('**/*',async socket=>{
        await socket.close({code:1008,reason:'Lu read-only browser'});
      });
      if(this.publicSearchOnly)await this.context.setOffline(true);
      await this.context.route('**/*', async route => {
        try {
          if(this.publicSearchOnly||this.publicSearchAttempted)throw new BrowserGateError('PUBLIC_SEARCH_NETWORK_BLOCKED');
          approvedUrl(route.request().url(), this.origins);
          await route.continue();
        } catch { await route.abort(); }
      });
      this.page = this.context.pages()[0] ?? await this.context.newPage();
      if (this.visibilityMode === 'automatic') {
        await this.#setWindowVisibility('HIDDEN');
      } else {
        this.windowVisibility = 'VISIBLE';
      }
    } catch (error) {
      // A failed origin firewall or page creation must never leave a
      // headful Chromium session behind (including a host-owned CDP attach).
      try { await this.close(); } catch { /* preserve original error */ }
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
  async #publicSearch(value) {
    this.publicSearchAttempted=true;
    try {
      const url=exactPublicSearchUrl(value);
      // Never reuse the persistent profile or its service worker/cookie store.
      // Production always attaches to the already OS-owned Chromium process.
      if(!this.connectedBrowser||typeof this.connectedBrowser.newContext!=='function')
        throw new BrowserGateError('PUBLIC_SEARCH_ISOLATION_UNAVAILABLE');
      const fetched=await this.publicSearchFetcher(url.href);
      if(fetched.status!==200||!Buffer.isBuffer(fetched.body)||!fetched.body.length||fetched.body.length>SEARCH_MAX_BYTES||
          createHash('sha256').update(fetched.body).digest('hex')!==fetched.bodySha256)
        throw new BrowserGateError('PUBLIC_SEARCH_INVALID_RESPONSE');
      const context=await this.connectedBrowser.newContext({javaScriptEnabled:false,
        serviceWorkers:'block',acceptDownloads:false,offline:true,viewport:{width:1280,height:900}});
      this.publicSearchContext=context;
      await context.routeWebSocket('**/*',socket=>socket.close({code:1008,reason:'Offline search document'}));
      await context.route('**/*',route=>route.abort());
      const page=await context.newPage();
      this.page=page;
      // Parse in an inert detached document in real Chromium, not an active
      // provider page. No response script, link, iframe or form is attached
      // to the live document; the entire isolated context is offline.
      const source=new TextDecoder('utf-8',{fatal:true}).decode(fetched.body);
      const parsed=await page.evaluate(html=>{
        const doc=new DOMParser().parseFromString(html,'text/html');
        doc.querySelectorAll('script,style,template,noscript').forEach(el=>el.remove());
        const gate=doc.querySelector('iframe[src*="/recaptcha/api2"],iframe[src*="hcaptcha.com"],iframe[src*="challenges.cloudflare.com"],[data-sitekey].h-captcha')?'CAPTCHA_REQUIRED':doc.querySelector('input[type="password"]')?'LOGIN_REQUIRED':null;
        return {bodyText:(doc.body?.textContent||'').slice(0,120000),gate,
          candidates:Array.from(doc.querySelectorAll('a:has(h3)')).slice(0,100).map(a=>({
            href:(a.getAttribute('href')||'').slice(0,8193),title:(a.querySelector('h3')?.textContent||'').slice(0,240),
            snippet:(a.parentElement?.parentElement?.textContent||'').slice(0,320)}))};
      },source);
      const gate=parsed.gate||challengeKind(parsed.bodyText)||(/before you continue to google|consent required/i.test(parsed.bodyText)?'CONSENT_REQUIRED':null);
      if(gate)throw new BrowserGateError('PUBLIC_SEARCH_'+gate);
      const candidates=parsed.candidates;
      const results=[];const seen=new Set();
      for(const candidate of candidates){
        const target=displayedSearchTarget(candidate.href,url.href);
        if(!target||!candidate.title.trim()||seen.has(target))continue;
        seen.add(target);
        results.push(`${candidate.title.trim().slice(0,240)}\nDisplayed link (not opened): ${target}\n${candidate.snippet.trim().slice(0,320)}`);
        if(results.length===8)break;
      }
      if(!results.length)throw new BrowserGateError('PUBLIC_SEARCH_NO_RESULTS');
      const text=results.join('\n\n');
      await page.evaluate(text=>{document.body.textContent=text;},text);
      this.publicSearchReceipt={url:url.href,text,transport:'pinned-https-v1',httpStatus:200,
        bodySha256:fetched.bodySha256,observedSha256:createHash('sha256').update(text).digest('hex'),resultCount:results.length};
      return {status:'SEARCH_OBSERVED',url:url.href,httpStatus:200,publicSearch:this.#searchEvidence()};
    }catch(e){
      this.publicSearchReceipt=null;
      if(this.publicSearchContext){await this.publicSearchContext.close().catch(()=>{});this.publicSearchContext=null;}
      return {status:'SEARCH_UNAVAILABLE',reason:e instanceof BrowserGateError?e.code:'PUBLIC_SEARCH_FAILED'};
    }
  }
  #searchEvidence(){
    const {transport,httpStatus,bodySha256,observedSha256,resultCount}=this.publicSearchReceipt;
    return {transport,httpStatus,bodySha256,observedSha256,resultCount};
  }
  async execute(command) {
    if (!this.page) throw new BrowserGateError('BROWSER_NOT_STARTED');
    if (!command || typeof command !== 'object' || Array.isArray(command)) {
      throw new BrowserGateError('INVALID_COMMAND');
    }
    const op = command.op;
    if (op === 'status') {
      return {status:this.paused ? 'NEEDS_HUMAN' : 'READY',reason:this.paused ?? null,
        url:this.publicSearchAttempted&&!this.publicSearchReceipt?null:this.page.url(),windowVisibility:this.windowVisibility,visibilityMode:this.visibilityMode};
    }
    if(this.publicSearchAttempted){
      if(op==='read'&&this.publicSearchReceipt&&this.page.url()==='about:blank'){
        const {url,text}=this.publicSearchReceipt;
        return {status:'READ',url,text,truncated:false,publicSearch:this.#searchEvidence()};
      }
      throw new BrowserGateError('PUBLIC_SEARCH_SCOPE_EXHAUSTED');
    }
    if(this.publicSearchOnly&&op!=='navigate')throw new BrowserGateError('PUBLIC_SEARCH_SCOPE_EXHAUSTED');
    if (op === 'human_resolved') {
      if (!this.paused) throw new BrowserGateError('NO_PENDING_CHALLENGE');
      // The user interacts with the existing visible browser. Re-check current
      // page instead of blindly assuming the challenge has been completed.
      approvedUrl(this.page.url(),this.origins);
      const text = await this.page.locator('body').innerText({timeout:5000});
      if (await inspectHumanGate(this.page,text)) throw new BrowserGateError('CHALLENGE_STILL_PRESENT');
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
    // Browser input may never continue on a redirect, popup or document
    // outside the trusted origin grants. Context.route is a second,
    // independent network fence; current-page checks protect interaction.
    if (!['navigate','show_browser','hide_browser'].includes(op)) {
      approvedUrl(this.page.url(),this.origins);
    }
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
      if(this.publicSearchOnly)return this.#publicSearch(url.href);
      const response = await this.page.goto(url.href,{waitUntil:'domcontentloaded',timeout:20000});
      approvedUrl(this.page.url(),this.origins);
      const text = await this.page.locator('body').innerText({timeout:5000})
        .catch(()=>{throw new BrowserGateError('CHALLENGE_STATE_UNAVAILABLE');});
      const challenge = await inspectHumanGate(this.page,text);
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
      // History navigation can land on an authentication or CAPTCHA gate;
      // do not silently continue autonomous browsing in that state.
      const text=await this.page.locator('body').innerText({timeout:5000})
        .catch(()=>{throw new BrowserGateError('CHALLENGE_STATE_UNAVAILABLE');});
      const challenge=await inspectHumanGate(this.page,text);
      if(challenge) return this.#requestHuman(challenge);
      return {status:'NAVIGATED',url:this.page.url()};
    }
    if (op === 'accessibility') {
      // The accessibility path is not an escape hatch around read()'s
      // CAPTCHA/login takeover gate.
      const text=await this.page.locator('body').innerText({timeout:5000})
        .catch(()=>{throw new BrowserGateError('CHALLENGE_STATE_UNAVAILABLE');});
      const challenge=await inspectHumanGate(this.page,text);
      if(challenge)return this.#requestHuman(challenge);
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
      const challenge=await inspectHumanGate(this.page,text);
      if (challenge) {
        return this.#requestHuman(challenge);
      }
      return {status:'READ',url:this.page.url(),text:text.slice(0,131072),truncated:text.length>131072};
    }
    if (op === 'screenshot') {
      // Prevent screenshot() from bypassing human challenge handoff.
      const text=await this.page.locator('body').innerText({timeout:5000})
        .catch(()=>{throw new BrowserGateError('CHALLENGE_STATE_UNAVAILABLE');});
      const challenge=await inspectHumanGate(this.page,text);
      if(challenge)return this.#requestHuman(challenge);
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
    if(this.publicSearchContext){await this.publicSearchContext.close();this.publicSearchContext=null;}
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
  // SEC-06C root-owned mediator must prove this exact Node worker actually
  // initialized its parser and lifetime pipe BEFORE admitting the pair.
  // Normal desktop/standalone protocol remains unchanged (no unsolicited
  // message without the first-party mediator's fixed env contract).
  if(process.env.LU_SEC06C_STARTUP_ACK==='1') {
    process.stdout.write('LU_SEC06C_NODE_READY_V1\n');
  }
  await new Promise(resolve=>input.once('close',resolve));
  bridge.close();
  await queue;
  await session.close();
}
if (process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  main().catch(() => {process.exitCode=1;});
}
