import React,{useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {emit} from '@tauri-apps/api/event';
import {CHAT_APPEARANCE_PRESETS,safeAppearance} from './message_interactions.mjs';

// These roots mount INSTEAD of App(), not alongside it. Separate Tauri
// WebviewWindows must never subscribe to chat-generation listeners, start
// background tasks, own chat drafts, or inherit the main composer state.
const SETTINGS_SECTIONS=[
  ['general','General & appearance'],
  ['models','Model-specific'],
  ['runtime','Lu runtime'],
  ['tools','Tools & permissions'],
  ['research','Browser & research'],
  ['memory','Memory & ANSF'],
  ['privacy','Privacy & Observe'],
  ['huggingface','Hugging Face'],
  ['diagnostics','Diagnostics']
];
const errorText=error=>String(error?.message||error||'Native operation unavailable');
function NativeWindowFrame({title,subtitle,children}){
  const [closing,setClosing]=useState(false);
  useEffect(()=>{
    const handle=event=>{
      if(event.key==='Escape'){
        event.preventDefault();
        void invoke('lu_close_aux_window').catch(()=>{});
      }
    };
    window.addEventListener('keydown',handle);
    return ()=>window.removeEventListener('keydown',handle);
  },[]);
  return <div className="nativeWindowRoot">
    <header className="nativeWindowHeader">
      <div><h1>{title}</h1><small>{subtitle}</small></div>
      <button type="button" className="subtleButton" disabled={closing}
        aria-label={'Close '+(title==='Settings'?'settings':'model library')}
        onClick={async()=>{
          setClosing(true);
          try{await invoke('lu_close_aux_window');}
          catch{setClosing(false);}
        }}>Close</button>
    </header>
    {children}
  </div>;
}
function Section({title,children}){
  return <section className="nativePreferenceSection"><h2>{title}</h2>{children}</section>;
}
function Readiness({value}){
  return <span className="nativeReadiness">{value||'Not independently verified'}</span>;
}
const bytesGiB=value=>Number.isFinite(value)&&value>=0?(value/1073741824).toFixed(2)+' GiB':'Not measured';

export function SettingsWindow(){
  const [section,setSection]=useState('general');
  const [chats,setChats]=useState([]);
  const [chatId,setChatId]=useState('');
  const [runtime,setRuntime]=useState(null);
  const [backends,setBackends]=useState([]);
  const [browserVisibility,setBrowserVisibility]=useState('automatic_hidden');
  const [browserRuntime,setBrowserRuntime]=useState(null);
  const [memory,setMemory]=useState(null);
  const [observation,setObservation]=useState(null);
  const [appearance,setAppearance]=useState(null);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  const [modelList,setModelList]=useState([]);
  const [memorySample,setMemorySample]=useState(null);
  const [capabilityStatus,setCapabilityStatus]=useState([]);
  const [vision,setVision]=useState(null);
  const [visionBusy,setVisionBusy]=useState(false);
  const [captureWorkerReady,setCaptureWorkerReady]=useState(false);
  const [captureTaskId,setCaptureTaskId]=useState(null);
  const [captureLocalTaskId,setCaptureLocalTaskId]=useState(null);
  const [captureReceipt,setCaptureReceipt]=useState(null);
  const [observeNotes,setObserveNotes]=useState([]);
  const [observeHandoff,setObserveHandoff]=useState(null);

  async function refresh(){
    const results=await Promise.allSettled([
      invoke('lu_status'),invoke('lu_model_backend_options'),invoke('lu_chats'),
      invoke('lu_browser_visibility'),invoke('lu_browser_runtime_status'),
      invoke('lu_list_models'),invoke('lu_native_route_catalog'),
      invoke('lu_vision_capability')
    ]);
    const setters=[setRuntime,setBackends,v=>setChats(v?.chats||[]),
      setBrowserVisibility,setBrowserRuntime,setModelList,setCapabilityStatus,setVision];
    results.forEach((result,i)=>{if(result.status==='fulfilled')setters[i](result.value);});
    if(results.every(x=>x.status==='rejected')){
      setError('Native Lu settings are unavailable. No preference was changed.');
    }
  }
  useEffect(()=>{
    void refresh();
    void invoke('lu_observe_capture_available').then(setCaptureWorkerReady)
      .catch(()=>setCaptureWorkerReady(false));
  },[]);
  useEffect(()=>{
    let alive=true;
    if(!chatId){setObservation(null);setAppearance(null);return ()=>{alive=false};}
    void Promise.allSettled([
      invoke('lu_observe_status',{chatId}),
      invoke('lu_message_decorations',{chatId,seqs:[]})
    ]).then(([obs,decor])=>{
      if(!alive)return;
      if(obs.status==='fulfilled')setObservation(obs.value);else setObservation(null);
      if(decor.status==='fulfilled')setAppearance(decor.value?.appearance||null);
    });
    return ()=>{alive=false};
  },[chatId]);
  async function act(label,perform){
    if(busy)return;
    setBusy(true);setError('');setNotice('');
    try{
      const result=await perform();
      if(result!==null&&result!==false)setNotice(label);
      return result;
    }catch(e){setError(errorText(e));return null;}
    finally{setBusy(false);}
  }
  async function verifyNativeVision(){
    if(!chatId||!runtime?.sessionId||visionBusy||busy)return;
    setVisionBusy(true);setError('');setNotice('');
    try{
      const result=await invoke('lu_vision_run_probe',{chatId});
      if(result?.sessionId!==runtime.sessionId||
         result?.modelRef!==runtime.modelRef){
        throw Error('Native pixel probe returned evidence for another loaded model; result discarded');
      }
      setVision(result);
      setNotice(result.status==='verified'
        ?'Four native pixel-inference challenges passed for this exact model session. Real-task accuracy remains unverified.'
        :'Native image probe did not establish visual competence.');
    }catch(e){setError(errorText(e));}
    finally{setVisionBusy(false);}
  }
  async function chooseBackend(backend){
    await act('Backend preference saved for next Lu restart.',async()=>{
      const saved=await invoke('lu_model_select_backend',{backend});
      if(saved)setBackends(await invoke('lu_model_backend_options'));
      return saved;
    });
  }
  async function changeObservation(action){
    if(!chatId)return;
    await act('Native observation status updated.',async()=>{
      const view=await invoke('lu_observe_'+action,{chatId});
      setObservation(view);return view;
    });
  }
  async function captureOneShot(foregroundOnly=false,primaryDisplay=false,localImage=false){
    if(!chatId||!captureWorkerReady||observation?.mode!=='active'||busy)return;
    await act('One-shot capture requested; read the native receipt before trusting any result.',async()=>{
      const id=await invoke('lu_observe_capture_one_shot',{
        chatId,foregroundOnly,primaryDisplay,localImage
      });
      setCaptureTaskId(id);
      setCaptureLocalTaskId(localImage?id:null);
      setCaptureReceipt(null);
      return id;
    });
  }
  async function inspectCapture(){
    if(!chatId||!captureTaskId)return;
    await act('Original capture receipt checked.',async()=>{
      const receipt=await invoke('lu_observe_capture_receipt',{chatId});
      setCaptureReceipt(receipt);
      return receipt;
    });
  }
  async function attachLocalCapture(){
    if(!chatId||captureLocalTaskId!==captureTaskId||
       observation?.mode!=='active'||
       !captureReceipt?.sourceIdentityAttested||
       !captureReceipt?.pixelsRetained)return;
    await act('Approved frame sent to this conversation as a local-only attachment.',async()=>{
      const image=await invoke('lu_observe_capture_local_image',{chatId});
      if(!image?.imageRef)throw Error('Native image transfer did not return an admitted image');
      // This renderer event grants nothing; lu_generate independently checks
      // host-owned imageRef, conversation ownership and local egress policy.
      await emit('lu-observed-image-admitted',{
        chatId,imageRef:image.imageRef,mimeType:image.mimeType
      });
      setCaptureLocalTaskId(null);
      return image;
    });
  }
  async function inspectAwareness(){
    if(!chatId)return;
    await act('Original local awareness notes reloaded.',async()=>{
      const notes=await invoke('lu_observe_notes',{chatId});
      setObserveNotes(notes);return notes;
    });
  }
  async function inspectHandoff(){
    if(!chatId)return;
    await act('Read-only observation handoff prepared for human review.',async()=>{
      const handoff=await invoke('lu_observe_handoff',{chatId});
      setObserveHandoff(handoff);return handoff;
    });
  }
  async function changeAppearance(preset){
    if(!chatId)return;
    await act('This conversation appearance was saved.',async()=>{
      const result=await invoke('lu_set_chat_appearance',{
        chatId,appearance:safeAppearance(preset)
      });
      setAppearance(result);
      await emit('lu-chat-appearance-changed',{chatId}).catch(()=>{});
      return result;
    });
  }
  async function openModels(){
    await act('Model Library opened.',()=>invoke('lu_open_aux_window',{kind:'model-library'}));
  }
  return <NativeWindowFrame title="Settings" subtitle="Lu preferences, privacy and model configuration">
    <div className="nativeWindowSplit" aria-label="Lu settings">
      <nav className="nativeSettingsNavigation" aria-label="Settings sections">
        {SETTINGS_SECTIONS.map(([key,label])=>
          <button type="button" key={key} aria-current={section===key?'page':undefined}
            className={section===key?'selected':''} onClick={()=>setSection(key)}>{label}</button>)}
      </nav>
      <main className="nativeSettingsContent">
        {error&&<p className="nativeError" role="alert">{error}</p>}
        {notice&&<p className="nativeNotice" role="status">{notice}</p>}
        {section==='general'&&<Section title="General & appearance">
          <p>The main Lu window is for conversations, folders and search. These preferences do not enter model prompts or authorize tools.</p>
          <label className="nativeField">Conversation for appearance
            <select value={chatId} onChange={e=>setChatId(e.target.value)}>
              <option value="">Select conversation…</option>
              {chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title}</option>)}
            </select>
          </label>
          {chatId&&<>
            <div className="nativeChoiceRow" aria-label="Conversation color presets">
              {CHAT_APPEARANCE_PRESETS.map(p=><button type="button" key={p.name}
                disabled={busy} onClick={()=>void changeAppearance(p)}>{p.name}</button>)}
            </div>
            <p>Current background: {appearance?.background||'default'} · Your bubbles: {appearance?.userBubble||'default'} · Lu bubbles: {appearance?.assistantBubble||'default'}</p>
          </>}
        </Section>}
        {section==='models'&&<Section title="Model-specific settings">
          <p>Model identities and compatibility are verified by Lu's native GGUF/MMProj preflight. Names or metadata alone never attest inference, vision or hardware support.</p>
          <p>Local registered models: {modelList.length}. Active model reference: {runtime?.modelRef||'none'}.</p>
          <button className="primaryButton" type="button" onClick={()=>void openModels()}>Open Model Library</button>
          <div className="nativeOption">
            <strong>Actual multimodal inference</strong>
            <p role="status">Vision: {vision?.status||'unverified'} — {vision?.detail||'No native result is available.'}</p>
            <label className="nativeField">Conversation for independently witnessed pixel probe
              <select value={chatId} onChange={e=>setChatId(e.target.value)}>
                <option value="">Select conversation…</option>
                {chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title}</option>)}
              </select>
            </label>
            <button type="button" disabled={!chatId||!runtime?.sessionId||busy||visionBusy}
              onClick={()=>void verifyNativeVision()}>
              {visionBusy?'Running actual GGUF/MMProj inference…':'Verify native pixel inference…'}
            </button>
            <p>Fresh OS approval is required. Lu generates four private randomized-color PNG challenges, runs them through the loaded GGUF/MMProj and checks each actual response. A missing projector, failed model response, or incompatible backend is never reported as verified vision.</p>
          </div>
          <p>Per-model context and prompt templates will appear here after model-specific runtime adapters are independently accepted. Unsupported settings are not exposed as working controls.</p>
        </Section>}
        {section==='runtime'&&<Section title="Lu runtime">
          <p>Choose the locally packaged engine for the next start. Saving a preference never grants GPU access or restarts an active session.</p>
          <div className="nativeOptionList">
            {backends.map(option=><article key={option.backend} className="nativeOption">
              <strong>{option.backend.toUpperCase()} {option.selectedOnNextStart?'· selected':''}</strong>
              <Readiness value={option.packaged?'Packaged source-pinned engine':'Not packaged'}/>
              <p>{option.detail}</p>
              {option.selectable&&<button type="button" disabled={busy||option.selectedOnNextStart}
                onClick={()=>void chooseBackend(option.backend)}>Use on next start</button>}
            </article>)}
          </div>
        </Section>}
        {section==='tools'&&<Section title="Tools & permissions">
          <p>These are host-owned capability ROUTES, not automatic grants. Any compatible model can receive a typed schema only after Lu binds an original human turn, a reviewed resource and the specific tool permission. Model text, extensions and settings cannot create authority.</p>
          <div className="nativeOptionList">
            {capabilityStatus.map(cap=><article key={cap.id} className="nativeOption">
              <strong>{cap.name}</strong> <Readiness value={cap.status}/>
              <p>{cap.description}</p>
              <small>{cap.authorization}</small>
            </article>)}
          </div>
          <p>Worker execution requires original-turn authorization, bounded input/output, cancellation, durable receipts and independent verification. Not every provider implements Lu's current tool-response protocol.</p>
        </Section>}
        {section==='research'&&<Section title="Browser & research">
          <p>Browser sessions are read-only, bound to explicitly approved public HTTPS origins, and refused when an independently reviewed browser runtime is not packaged.</p>
          <label className="nativeField">Window visibility
            <select disabled={busy} value={browserVisibility}
              onChange={event=>void act('Browser preference saved.',async()=>{
                const next=await invoke('lu_set_browser_visibility',{mode:event.target.value});
                setBrowserVisibility(next);return next;
              })}>
              <option value="automatic_hidden">Automatic — visible for approval</option>
              <option value="always_visible">Always visible</option>
            </select>
          </label>
          <Readiness value={browserRuntime?.reason||'Browser runtime not independently verified'}/>
          <p>Research source checking, cited page reading and retrieved documents are initiated through the conversation; they are not settings operations.</p>
        </Section>}
        {section==='memory'&&<Section title="Memory & ANSF">
          <p>Lu stores original conversations, user corrections and independently verified native effects in host-owned durable memory. Retrieved history is context, not fresh permission to run tools.</p>
          <p>Search past conversations from the main Lu sidebar. The original-source history reader remains a read-only conversation view. Semantic indexing and recovery must report genuine source and receipt status.</p>
        </Section>}
        {section==='privacy'&&<Section title="Privacy & Observe">
          <p>Observe & Learn is OFF by default. Enabling it requires both existing native OS confirmations. Observation never grants general network egress.</p>
          <label className="nativeField">Conversation to observe
            <select value={chatId} onChange={e=>setChatId(e.target.value)}>
              <option value="">Select conversation…</option>
              {chats.map(c=><option key={c.chatId} value={c.chatId}>{c.title}</option>)}
            </select>
          </label>
          <p role="status">Observation: {observation?.mode||'off or unavailable'}</p>
          {chatId&&<div className="nativeChoiceRow">
            {(!observation||observation.mode==='off')&&
              <button type="button" disabled={busy} onClick={()=>void changeObservation('start')}>Request native consent</button>}
            {observation?.mode==='active'&&
              <button type="button" disabled={busy} onClick={()=>void changeObservation('pause')}>Pause</button>}
            {observation?.mode==='paused'&&
              <button type="button" disabled={busy} onClick={()=>void changeObservation('resume')}>Resume</button>}
            {observation&&observation.mode!=='off'&&
              <button type="button" disabled={busy} onClick={()=>void changeObservation('stop')}>Stop</button>}
            <button type="button" disabled={busy} onClick={()=>void act('Native deletion review completed.',()=>{
              return invoke('lu_observe_forget_notes',{chatId});
            })}>Review forgetting notes</button>
          </div>}
          {observation?.mode==='active'&&captureWorkerReady&&<>
            <h3>One-shot Windows capture · additional consent required</h3>
            <p>Each button opens a fresh OS review. A receipt-only frame is fully redacted. Local multimodal images are never uploaded or shared with research.</p>
            <div className="nativeChoiceRow">
              <button type="button" disabled={busy} onClick={()=>void captureOneShot(true,false,false)}>Capture selected foreground app (4s)</button>
              <button type="button" disabled={busy} onClick={()=>void captureOneShot(false,true,false)}>Capture primary display (redacted)</button>
              <button type="button" disabled={busy} onClick={()=>void captureOneShot(false,false,false)}>Select window/display · capture one frame</button>
            </div>
            <div className="nativeChoiceRow">
              <button type="button" disabled={busy} onClick={()=>void captureOneShot(true,false,true)}>Capture foreground app for local GGUF image</button>
              <button type="button" disabled={busy} onClick={()=>void captureOneShot(false,true,true)}>Capture primary display for local GGUF image</button>
            </div>
          </>}
          {!captureWorkerReady&&<p>Verified Windows capture worker not installed. Lu cannot start screen capture.</p>}
          {captureTaskId&&<div className="nativeOption">
            <strong>Capture task: {captureTaskId}</strong>
            <button type="button" disabled={busy} onClick={()=>void inspectCapture()}>Inspect original native frame receipt</button>
            {captureReceipt&&<p>State: {captureReceipt.state||'unavailable'}.
              {captureReceipt.fullyRedactedReceipt?' Receipt-only pixels fully blanked.':''}
              {captureReceipt.sourceIdentityAttested?' Foreground window/display identity checked.':' Source identity not independently attested.'}
              No result here proves visual understanding.</p>}
            {captureLocalTaskId===captureTaskId&&captureReceipt?.pixelsRetained&&
              captureReceipt?.sourceIdentityAttested&&observation?.mode==='active'&&
              <button type="button" disabled={busy} onClick={()=>void attachLocalCapture()}>
                Attach approved local frame to chat
              </button>}
          </div>}
          <h3>Local awareness notes</h3>
          <div className="nativeChoiceRow">
            <button type="button" disabled={busy||!chatId} onClick={()=>void inspectAwareness()}>Review notes</button>
            <button type="button" disabled={busy||!chatId} onClick={()=>void inspectHandoff()}>Prepare large-model review handoff</button>
          </div>
          {observeNotes.length>0&&<p>{observeNotes.length} local awareness entries. Unverified UI hints are not test outcomes or automatic learning.</p>}
          {observeHandoff&&<section className="nativeOption">
            <strong>Read-only original-source handoff</strong>
            <textarea aria-label="Original observation handoff" readOnly rows={6}
              value={observeHandoff.reviewDraft||''}/>
            <p>Nothing is sent automatically. Copy and approve in the conversation before using any of these observations.</p>
          </section>}
          <small>One-shot capture requires genuine Windows Graphics Capture, OS-confirmed source and the separately reviewed worker. No screenshot is taken automatically.</small>
        </Section>}
        {section==='huggingface'&&<Section title="Hugging Face model discovery">
          <p>Browse remote GGUF metadata in Model Library. An installed SHA-pinned HTTPS client can start a separately OS-reviewed transfer, with cancellation, interruption recovery, exact-range resume and independent full-file SHA-256 readback. Availability is checked at runtime. A completed transfer is never automatically loaded or treated as proven inference.</p>
          <button className="primaryButton" type="button" onClick={()=>void openModels()}>Open Model Library · Discover</button>
        </Section>}
        {section==='diagnostics'&&<Section title="Diagnostics">
          <p>Active model: {runtime?.modelRef||'none'} · Inference phase: {runtime?.phase||'unavailable'}</p>
          <button type="button" className="subtleButton" disabled={busy} onClick={()=>void act('Native process memory sampled.',async()=>{
            const sample=await invoke('lu_load_memory_sample');setMemorySample(sample);return sample;
          })}>Sample RAM / VRAM</button>
          <p>Working-set RAM: {bytesGiB(memorySample?.workingSetRamBytes)}</p>
          <p>Dedicated GPU: {bytesGiB(memorySample?.dedicatedGpuBytes)}</p>
          <p>Tensor loading percentage unavailable until exact backend tensor-progress measurements exist. OS process counters do not prove Vulkan offload.</p>
        </Section>}
      </main>
    </div>
  </NativeWindowFrame>;
}

export function ModelLibraryWindow(){
  const [tab,setTab]=useState('installed');
  const [models,setModels]=useState([]);
  const [runtime,setRuntime]=useState(null);
  const [selected,setSelected]=useState('');
  const [inspection,setInspection]=useState(null);
  const [query,setQuery]=useState('');
  const [providerQuery,setProviderQuery]=useState('');
  const [hits,setHits]=useState([]);
  const [review,setReview]=useState(null);
  const [transfers,setTransfers]=useState([]);
  const [transferReady,setTransferReady]=useState(false);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  async function refresh(){
    const [list,status]=await Promise.all([invoke('lu_list_models'),invoke('lu_status')]);
    setModels(list);setRuntime(status);
    setSelected(value=>list.some(m=>m.modelRef===value)?value:status.modelRef||list[0]?.modelRef||'');
  }
  async function refreshTransfers(){
    const views=await invoke('lu_model_transfer_recent');
    setTransfers(views);
    return views;
  }
  useEffect(()=>{
    let alive=true;
    void refresh().catch(e=>setError(errorText(e)));
    void invoke('lu_model_transfer_ready')
      .then(v=>{if(alive)setTransferReady(Boolean(v));})
      .catch(()=>{if(alive)setTransferReady(false);});
    void refreshTransfers().catch(()=>{});
    const interval=setInterval(()=>{
      if(alive)void refreshTransfers().catch(()=>{});
    },1600);
    return ()=>{alive=false;clearInterval(interval);};
  },[]);
  useEffect(()=>{
    let alive=true;
    if(!selected){setInspection(null);return ()=>{alive=false};}
    void invoke('lu_model_preflight',{modelRef:selected})
      .then(view=>{if(alive)setInspection(view);})
      .catch(error=>{if(alive){setInspection(null);setError(errorText(error));}});
    return ()=>{alive=false};
  },[selected,models]);
  async function act(message,fn){
    if(busy)return;
    setBusy(true);setError('');setNotice('');
    try{
      const value=await fn();
      if(value!==null&&value!==false)setNotice(message);
      return value;
    }catch(e){setError(errorText(e));return null;}
    finally{setBusy(false);}
  }
  async function mutated(fn){
    return act('Native model catalog refreshed.',async()=>{
      const result=await fn();
      await refresh();
      await emit('lu-model-catalog-changed',{source:'model-library'}).catch(()=>{});
      return result;
    });
  }
  const current=models.find(x=>x.modelRef===selected);
  const filtered=models.filter(m=>m.filename.toLowerCase().includes(query.trim().toLowerCase()));
  const loaded=Boolean(runtime?.sessionId);
  return <NativeWindowFrame title="Model Library" subtitle="Local models and public Hugging Face metadata">
    <nav className="nativeModelTabs" aria-label="Model library tabs">
      <button aria-current={tab==='installed'?'page':undefined} onClick={()=>setTab('installed')}>Installed GGUF</button>
      <button aria-current={tab==='discover'?'page':undefined} onClick={()=>setTab('discover')}>Discover · Hugging Face</button>
    </nav>
    <main className="nativeModelContent" aria-label="Model Library">
      {error&&<p className="nativeError" role="alert">{error}</p>}
      {notice&&<p className="nativeNotice" role="status">{notice}</p>}
      {tab==='installed'&&<>
        <div className="nativeChoiceRow">
          <input aria-label="Search local models" type="search" maxLength={100}
            placeholder="Search installed models" value={query} onChange={e=>setQuery(e.target.value)}/>
          <button disabled={busy||loaded} onClick={()=>void mutated(()=>invoke('lu_rescan_models'))}>Rescan</button>
          <button disabled={busy} onClick={()=>void mutated(()=>invoke('lu_import_model'))}>Import GGUF…</button>
        </div>
        <div className="nativeModelGrid">
          {filtered.map(m=><button key={m.modelRef} type="button"
            className={'nativeModelCard '+(selected===m.modelRef?'selected':'')}
            aria-pressed={selected===m.modelRef} onClick={()=>setSelected(m.modelRef)}>
            <strong>{m.filename}</strong>
            <small>{m.projectorAttached?'GGUF + MMProj':'GGUF'} · {m.available?'Local file available':'Missing file'}</small>
          </button>)}
        </div>
        {models.length===0&&<p>No registered local GGUF models. Import from a native file picker or use a verified transfer manager when available.</p>}
        {models.length>0&&filtered.length===0&&<p>No models match this search.</p>}
        {current&&<Section title="Selected model">
          <h3>{current.filename}</h3>
          <p>{inspection?.modelRef===selected?inspection.detail:'Verifying native model metadata…'}</p>
          <p>Architecture: {inspection?.architecture||'unknown'} · GGUF v{inspection?.ggufVersion||'?'} · File size: {bytesGiB(inspection?.fileBytes)}</p>
          <p>Context claimed in metadata: {inspection?.reportedContextLength||'unknown'} · Projector: {current.projectorAttached?'attached':'not attached'}</p>
          <p>Current runtime: {runtime?.modelRef||'no model loaded'}. Model metadata cannot prove inference or vision competence.</p>
          <div className="nativeChoiceRow">
            <button disabled={busy||loaded||current.projectorAttached}
              onClick={()=>void mutated(()=>invoke('lu_attach_projector',{modelRef:selected}))}>Attach MMProj…</button>
            <button disabled={busy||loaded||!current.available||inspection?.modelRef!==selected||!inspection?.loadAllowed}
              onClick={()=>void mutated(()=>invoke('lu_load_model',{modelRef:selected}))}>Load model</button>
            <button disabled={busy||!loaded} onClick={()=>void mutated(()=>invoke('lu_unload_model'))}>Unload active model</button>
          </div>
        </Section>}
      </>}
      {tab==='discover'&&<>
        <div className="nativeChoiceRow">
          <input aria-label="Search Hugging Face public GGUF repositories" type="search" maxLength={96}
            placeholder="Model / author" value={providerQuery} onChange={e=>setProviderQuery(e.target.value)}/>
          <button disabled={busy||!providerQuery.trim()} onClick={()=>void act('Provider search completed.',async()=>{
            setReview(null);
            const items=await invoke('lu_model_provider_search',{query:providerQuery.trim()});
            setHits(items);return items;
          })}>Search catalog</button>
        </div>
        <p>Model discovery is metadata-only. {transferReady
          ?'Lu has an installed hash-pinned HTTPS transfer client. Each download and interrupted resume still needs a fresh OS confirmation, exact immutable asset and full verified bytes.'
          :'The independently pinned Lu transfer client is not available; remote download and resume are disabled.'}
          No successful download by itself admits a GGUF for inference or GPU loading.</p>
        <div className="nativeModelGrid">
          {hits.map(lead=><button key={lead.repoId+lead.revision} className="nativeModelCard"
            onClick={()=>void act('Immutable model metadata reviewed.',async()=>{
              const detail=await invoke('lu_model_provider_review',{
                repoId:lead.repoId,revision:lead.revision
              });
              if(detail.repoId!==lead.repoId||detail.revision!==lead.revision||
                 detail.downloadAvailable!==false){
                throw Error('Provider revision or transfer status changed; no model was admitted');
              }
              setReview(detail);return detail;
            })}>
            <strong>{lead.repoId}</strong>
            <small>Revision {lead.revision?.slice(0,12)||'unknown'} · License {lead.licenseLabel||'unknown'}</small>
          </button>)}
        </div>
        <Section title="Verified model transfers & recovery">
          <p>These are original host-owned transfer receipts. Downloaded bytes are distinct from GGUF compatibility, model-load RAM/VRAM or inference verification.</p>
          <div className="nativeChoiceRow">
            <button type="button" disabled={busy}
              onClick={()=>void act('Original transfer receipts refreshed.',refreshTransfers)}>
              Refresh transfers
            </button>
          </div>
          {transfers.map(t=>{
            const measuredPercent=t.totalBytes>0
              ?Math.min(100,Math.floor(100*t.receivedBytes/t.totalBytes)):0;
            const running=t.state==='RUNNING'||t.state==='REVIEWED';
            const interrupted=t.state==='INTERRUPTED_REVIEW_REQUIRED';
            const published=t.state==='VERIFIED_AT_PUBLICATION';
            return <article key={t.transferId} className="nativeOption">
              <strong>{t.filename}</strong>
              <small>Immutable repository {t.repoId} · revision {t.revision.slice(0,12)}</small>
              <p>{t.state} · {t.receivedBytes.toLocaleString()} / {t.totalBytes.toLocaleString()} bytes · {measuredPercent}% received</p>
              <div role="progressbar" aria-label={'Measured transfer bytes for '+t.filename}
                aria-valuemin={0} aria-valuemax={100} aria-valuenow={measuredPercent}
                className="nativeTransferProgress">
                <span style={{width:measuredPercent+'%'}}/>
              </div>
              <small>Expected SHA-256: {t.expectedSha256}</small>
              <p>{t.message}</p>
              <div className="nativeChoiceRow">
                {running&&<button type="button" disabled={busy}
                  onClick={()=>void act('Cancellation signaled to owned transfer worker.',async()=>{
                    const signaled=await invoke('lu_model_transfer_cancel',{transferId:t.transferId});
                    await refreshTransfers();return signaled;
                  })}>Cancel download</button>}
                {interrupted&&<button type="button" disabled={busy||!transferReady}
                  onClick={()=>void act('Separately reviewed immutable transfer resume.',async()=>{
                    const resumed=await invoke('lu_model_transfer_resume',{transferId:t.transferId});
                    await refreshTransfers();return resumed;
                  })}>Review and resume…</button>}
                {published&&<button type="button" disabled={busy}
                  onClick={()=>void act('Current downloaded GGUF bytes independently reverified.',async()=>{
                    const verified=await invoke('lu_model_transfer_verify',{transferId:t.transferId});
                    await refreshTransfers();return verified;
                  })}>Verify full file SHA-256 now</button>}
                {published&&<button type="button" disabled={busy||loaded}
                  onClick={()=>void mutated(()=>invoke('lu_rescan_models'))}>
                  Admit GGUF metadata into Model Library
                </button>}
              </div>
            </article>
          })}
          {transfers.length===0&&<p>No managed model downloads or interrupted transfers have been recorded.</p>}
        </Section>
        {review&&<Section title={review.repoId}>
          <p>Immutable revision: {review.revision} · Provider-reported license: {review.licenseLabel||'unknown'}</p>
          {(review.files||[]).map(file=><article key={file.name} className="nativeOption">
            <strong>{file.name}</strong>
            <p>{bytesGiB(file.bytes)} · SHA-256: <code>{file.sha256}</code></p>
            <button type="button" disabled={busy||!transferReady}
              onClick={()=>void act('Fresh native download review completed.',async()=>{
                const started=await invoke('lu_model_transfer_start',{
                  repoId:review.repoId,revision:review.revision,
                  filename:file.name,sha256:file.sha256,bytes:file.bytes
                });
                await refreshTransfers();return started;
              })}>Review and download exact GGUF…</button>
          </article>)}
          <p>Only the native operator may authorize transfer. Lu never uses a model-supplied URL, writes outside its model library, resumes after restart without review or automatically loads downloaded weights.</p>
          {review.sourceUrl&&<a href={review.sourceUrl} target="_blank" rel="noopener noreferrer">View on Hugging Face</a>}
        </Section>}
      </>}
    </main>
  </NativeWindowFrame>;
}
