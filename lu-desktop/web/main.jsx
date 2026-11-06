import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { splitModelOutput } from './response_view.mjs';
import { fullEditChangeLines } from './edit_diff.mjs';
import { MarkdownView } from './markdown_view.jsx';
import {buildSubmittedPrompt, attachmentSummary, MAX_TEXT_FILES} from './attachment_drafts.mjs';
import {isPotentialImage, normalizedImageFile} from './image_attachment_input.mjs';
import {queuedJob, startedJob, tokenForJob, removeJob} from './generation_jobs.mjs';
import {nearTranscriptBottom, restoredScrollTop, isSendKey, isSearchKey, isNewChatKey, matchesChatTitle} from './chat_ui_behavior.mjs';

function MessageText({text, role='assistant'}) {
  if (role === 'user') return <div className="messageContent userText">{text}</div>;
  return <div className="messageContent">{splitModelOutput(text).map((part,i) =>
    part.kind === 'think' ? <details key={i} className="thinkBlock"><summary>Model reasoning</summary><MarkdownView text={part.text}/></details>
      : <MarkdownView key={i} text={part.text}/>)}</div>;
}
const MAX_TEXT_ATTACHMENT = 64 * 1024;
const MAX_IMAGE_ATTACHMENT = 8 * 1024 * 1024;

function App() {
  const [models, setModels] = useState([]);
  const [selectedModel, setSelectedModel] = useState('');
  const [runtime, setRuntime] = useState({ phase: 'Idle', modelRef: null, sessionId: null });
  const [chats, setChats] = useState([]);
  const [folders, setFolders] = useState([]);
  const [chatId, setChatId] = useState(null);
  const [history, setHistory] = useState([]);
  const [prompt, setPrompt] = useState('');
  const [images, setImages] = useState([]);
  const [imageViewer,setImageViewer] = useState(null);
  const [imageEdit,setImageEdit]=useState({mode:'resize',x:'0',y:'0',width:'',height:'',blockSize:'12',stroke:'3',color:'#ff5500'});
  const [imageExportBusy,setImageExportBusy]=useState(false);
  const [imageExportReceipt,setImageExportReceipt]=useState(null);
  const [imageHistoryView,setImageHistoryView]=useState({chatId:null,rows:[]});
  const [imageHistoryBusy,setImageHistoryBusy]=useState(false);
  const [visionCapability,setVisionCapability] = useState(null); // session diagnostic only, no model attestation
  const [observeView,setObserveView] = useState({mode:'off'}); // Lu-only semantic hints
  const [observeNotes,setObserveNotes] = useState([]);
  const [observeHandoff,setObserveHandoff] = useState(null);
  const [captureWorkerReady,setCaptureWorkerReady]=useState(false);
  const [captureTaskId,setCaptureTaskId]=useState(null);
  const [captureStarting,setCaptureStarting]=useState(false);
  const [browserVisibility,setBrowserVisibility] = useState('automatic_hidden');
  const [browserPreferenceError,setBrowserPreferenceError] = useState('');
  const [browserRuntime,setBrowserRuntime] = useState(null);
  const [browserOrigins,setBrowserOrigins]=useState('https://example.org');
  const [browserBusy,setBrowserBusy]=useState(false);
  const [browserReadBusy,setBrowserReadBusy]=useState(false);
  const [browserReadResult,setBrowserReadResult]=useState(null);
  const [browserReadError,setBrowserReadError]=useState('');
  const [researchAvailable,setResearchAvailable]=useState(false);
  const [researchQuery,setResearchQuery]=useState('');
  const [researchBusy,setResearchBusy]=useState(false);
  const [researchResult,setResearchResult]=useState(null);
  const [researchMetadataByChat,setResearchMetadataByChat]=useState({});
  const [researchDocumentsByChat,setResearchDocumentsByChat]=useState({});
  const [researchCampaignsByChat,setResearchCampaignsByChat]=useState({});
  const [researchCampaignCeiling,setResearchCampaignCeiling]=useState(120);
  const [researchCampaignBusy,setResearchCampaignBusy]=useState(false);
  const [researchClaimDraft,setResearchClaimDraft]=useState({
    sourceId:'',claim:'',quote:'',stance:'COMPLICATES',reasoning:''
  });
  const [researchClaimsByChat,setResearchClaimsByChat]=useState({});
  const [researchReportTitle,setResearchReportTitle]=useState('Research evidence report');
  const [researchReportSavedByChat,setResearchReportSavedByChat]=useState({});
  const [researchReportBusy,setResearchReportBusy]=useState(false);
  const [researchDocumentQuotes,setResearchDocumentQuotes]=useState({});
  const [researchDocumentPassages,setResearchDocumentPassages]=useState({});
  const [researchError,setResearchError]=useState('');
  const [browserEvidenceByChat,setBrowserEvidenceByChat]=useState({});
  const [browserEvidenceBusy,setBrowserEvidenceBusy]=useState(false);
  const [researchQuoteDraftByKey,setResearchQuoteDraftByKey]=useState({});
  const [researchQuoteSourceByKey,setResearchQuoteSourceByKey]=useState({});
  const [researchQuoteProofByKey,setResearchQuoteProofByKey]=useState({});
  const [researchQuoteBusy,setResearchQuoteBusy]=useState(false);
  const [textFiles, setTextFiles] = useState([]);
  const [jobs, setJobs] = useState({}); // bounded, per-chat host generation state
  const [readProposals, setReadProposals] = useState({}); // completed model proposals, never grants
  const [writeProposals, setWriteProposals] = useState({}); // inert completed model write previews
  const [writeRecovery, setWriteRecovery] = useState([]); // read-only durable evidence
  const [editProposals, setEditProposals] = useState({});
  const [nativeActionProposals,setNativeActionProposals]=useState({}); // inert Wave 3 review intents
  const [toolActivityOpen,setToolActivityOpen]=useState(false);
  const [toolHistoryByChat,setToolHistoryByChat]=useState({});
  const [toolProgressByChat,setToolProgressByChat]=useState({});
  const [turnCapabilitiesByChat,setTurnCapabilitiesByChat]=useState({});
  const [toolHistoryBusy,setToolHistoryBusy]=useState(false);
  const [editReviewed, setEditReviewed] = useState({}); // UI only; native dialog grants authority
  const [editRecovery, setEditRecovery] = useState([]);
  const [editObservations, setEditObservations] = useState({});
  const [rollbackReviewed, setRollbackReviewed] = useState({});
  const [rollbackHistory, setRollbackHistory] = useState([]);
  const [latestRestored, setLatestRestored] = useState(null);
  const [latestEdited, setLatestEdited] = useState(null);
  const [backgroundPaneOpen,setBackgroundPaneOpen] = useState(false);
  const [backgroundTasks,setBackgroundTasks] = useState([]);
  const [backgroundError,setBackgroundError] = useState('');
  const [backgroundFilter,setBackgroundFilter] = useState('active');
  const [backgroundTerminalNotice,setBackgroundTerminalNotice] = useState(null);
  const [autoBackgroundContinuation,setAutoBackgroundContinuation] = useState(false);
  const [working, setWorking] = useState(false);
  // An opt-in for exactly one ordinary Send. This boolean is never authority:
  // the host still requires an actual native directory picker.
  const [workspaceToolsNextSend,setWorkspaceToolsNextSend] = useState(false);
  // Explicit one-turn Browser opt-in; native dialog remains the grant.
  const [browserResearchNextSend,setBrowserResearchNextSend]=useState(false);
  const [modelLoadSeconds, setModelLoadSeconds] = useState(0);
  const [modelLoadTrace, setModelLoadTrace] = useState(null);
  const [lastHostLoadReply, setLastHostLoadReply] = useState(0);
  const [error, setError] = useState('');
  const [activity, setActivity] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [advancedSettingsOpen, setAdvancedSettingsOpen] = useState(false);
  const [documentSettings, setDocumentSettings] = useState(null);
  const [latestCreated, setLatestCreated] = useState(null);
  const [archiveReview, setArchiveReview] = useState(null);
  const [archiveReviewed, setArchiveReviewed] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const [archiveCancelRequested, setArchiveCancelRequested] = useState(false);
  const [latestArchiveExtract, setLatestArchiveExtract] = useState(null);
  const [documentComposerOpen, setDocumentComposerOpen] = useState(false);
  const [documentDraft, setDocumentDraft] = useState({format:'docx',title:'',content:''});
  const [documentCreateReview, setDocumentCreateReview] = useState(null);
  const [documentCreateReviewed, setDocumentCreateReviewed] = useState(false);
  const [documentCreateBusy, setDocumentCreateBusy] = useState(false);
  const [latestDocumentCreated, setLatestDocumentCreated] = useState(null);
  const [officeRevisionOpen, setOfficeRevisionOpen] = useState(false);
  const [officePatch, setOfficePatch] = useState({
    kind:'word_text',textIndex:0,slideNumber:1,sheetNumber:1,
    address:'A1',expectedOld:'',newValue:''
  });
  const [officeRevisionReview, setOfficeRevisionReview] = useState(null);
  const [officeRevisionReviewed, setOfficeRevisionReviewed] = useState(false);
  const [officeRevisionBusy, setOfficeRevisionBusy] = useState(false);
  const [latestOfficeRevision, setLatestOfficeRevision] = useState(null);
  const [officeBatchOpen, setOfficeBatchOpen] = useState(false);
  const [officeBatchDraft, setOfficeBatchDraft] = useState([
    {kind:'word_text',textIndex:0,slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''},
    {kind:'word_text',textIndex:1,slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''}
  ]);
  const [officeBatchReview, setOfficeBatchReview] = useState(null);
  const [officeBatchReviewed, setOfficeBatchReviewed] = useState(false);
  const [officeBatchBusy, setOfficeBatchBusy] = useState(false);
  const [latestOfficeBatch, setLatestOfficeBatch] = useState(null);
  const [fileTransferReview, setFileTransferReview] = useState(null);
  const [fileTransferReviewed, setFileTransferReviewed] = useState(false);
  const [fileTransferBusy, setFileTransferBusy] = useState(false);
  const [latestFileTransfer, setLatestFileTransfer] = useState(null);
  const [trashReview, setTrashReview] = useState(null);
  const [trashReviewed, setTrashReviewed] = useState(false);
  const [restoreReview, setRestoreReview] = useState(null);
  const [restoreReviewed, setRestoreReviewed] = useState(false);
  const [trashBusy, setTrashBusy] = useState(false);
  const [trashHistoryOpen, setTrashHistoryOpen] = useState(false);
  const [trashHistory, setTrashHistory] = useState([]);
  const [latestCustodyOutcome, setLatestCustodyOutcome] = useState(null);
  const [wave2RecoveryByChat,setWave2RecoveryByChat] = useState({});
  const [wave2RecoveryBusy,setWave2RecoveryBusy] = useState(false);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const [folderDraft, setFolderDraft] = useState(false);
  const [folderName, setFolderName] = useState('');
  const [expandedFolders, setExpandedFolders] = useState([]);
  const [composerHeight, setComposerHeight] = useState(130);
  const [composerCollapsed, setComposerCollapsed] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const [chatQuery, setChatQuery] = useState('');
  const [atBottom, setAtBottom] = useState(true);
  const transcriptRef = useRef(null);
  const scrollByChat = useRef(new Map());
  const followOutput = useRef(true);
  const searchRef = useRef(null);
  const composerRef = useRef(null);
  const modelMenuRef = useRef(null);
  const requestedChat = useRef(null);
  const dragSize = useRef(null);
  const fileInput = useRef(null);
  const currentChat = useRef(null);
  const pendingDrafts = useRef(new Map());
  const draftText = useRef('');
  const draftImages = useRef([]);
  const draftFiles = useRef([]);
  const previewUrls = useRef(new Set());
  const selected = models.find(m => m.modelRef === selectedModel);
  const loadedModel = models.find(m => m.modelRef === runtime.modelRef);
  const viewedDraftImage=imageViewer && images.find(im=>im.imageRef===imageViewer.imageRef);
  const loaded = runtime.phase === 'Ready' || runtime.phase === 'Generating';
  const busy = working || runtime.phase === 'Generating' || Object.keys(jobs).length > 0;
  const wave2FileBusy = archiveBusy || documentCreateBusy || officeRevisionBusy ||
    fileTransferBusy || trashBusy;
  const activeChat = chats.find(c => c.chatId === chatId);
  const matchingChats = chats.filter(c => matchesChatTitle(c, chatQuery));
  const visibleJob = chatId ? jobs[chatId] : null;
  const visibleStream = visibleJob?.text ?? '';
  const chatBusy = Boolean(visibleJob);
  const activeBackgroundTasks=backgroundTasks.filter(task=>
    ['PENDING','RUNNING','PAUSED'].includes(task.state));
  const shownBackgroundTasks=backgroundFilter==='active'
    ?activeBackgroundTasks:backgroundTasks;
  // An in-flight checkpoint is already visible in the live stream buffer.
  // Do not render that same unfinished assistant text twice after chat switch.
  const displayedHistory = visibleJob ? history.filter((entry,i) =>
    !(i === history.length - 1 && entry.role === 'assistant' && entry.interrupted && !entry.cancelled)) : history;
  useEffect(() => { draftText.current = prompt; draftImages.current = images; draftFiles.current = textFiles; }, [prompt, images, textFiles]);
  useEffect(() => {
    if (!(working && activity.startsWith('Loading'))) { setModelLoadSeconds(0); return; }
    const started = Date.now();
    setModelLoadSeconds(0);
    const timer = setInterval(() => setModelLoadSeconds(Math.floor((Date.now() - started) / 1000)), 1000);
    return () => clearInterval(timer);
  }, [working, activity]);
  useEffect(() => {
    if (!(working && activity.startsWith('Loading'))) {
      setModelLoadTrace(null);
      setLastHostLoadReply(0);
      return;
    }
    let mounted = true;
    let pending = false;
    async function sampleHost() {
      if (pending) return; // One outstanding probe; no unbounded request buildup.
      pending = true;
      try {
        const trace = await invoke('lu_load_diagnostics');
        if (mounted) {
          setModelLoadTrace(trace);
          setLastHostLoadReply(Date.now());
        }
      } catch {
        // A failed probe is not proof of a GPU or WebView deadlock.
        if (mounted) setLastHostLoadReply(0);
      } finally {
        pending = false;
      }
    }
    sampleHost();
    const timer = setInterval(sampleHost, 1500);
    return () => { mounted = false; clearInterval(timer); };
  }, [working, activity]);
  useEffect(() => () => { for (const url of previewUrls.current) URL.revokeObjectURL(url); }, []);
  useEffect(()=>{
    if(!settingsOpen)return;
    let active=true;
    invoke('lu_browser_runtime_status')
      .then(v=>{if(active)setBrowserRuntime(v);})
      .catch(e=>{if(active)setBrowserPreferenceError(String(e));});
    return()=>{active=false;};
  },[settingsOpen]);
  useEffect(()=>{
    if(!settingsOpen)return;
    invoke('lu_research_available')
      .then(v=>setResearchAvailable(Boolean(v)))
      .catch(()=>setResearchAvailable(false));
  },[settingsOpen]);
  async function runResearchDiscovery(page=1){
    const selectedPage=Number.isInteger(page)?page:1;
    if(!chatId||researchBusy||!researchAvailable
      ||selectedPage<1||selectedPage>5)return;
    const sourceChat=chatId;
    setResearchBusy(true);
    setResearchError('');
    setResearchResult(null);
    try{
      const result=await invoke('lu_research_openalex',{chatId:sourceChat,query:researchQuery,page:selectedPage});
      // Preserve the requesting chat as part of the displayed evidence scope.
      setResearchResult({...result,chatId:sourceChat});
    }catch(e){setResearchError(String(e));}
    finally{setResearchBusy(false);}
  }
  async function checkResearchMetadata(sourceId){
    if(!chatId||!researchAvailable||researchBusy||!sourceId)return;
    const sourceChat=chatId;
    setResearchBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_crossref',{chatId:sourceChat,sourceId});
      setResearchMetadataByChat(old=>({
        ...old,[sourceChat+'|'+sourceId]:result
      }));
    }catch(e){setResearchError(String(e));}
    finally{setResearchBusy(false);}
  }
  async function refreshResearchClaims(selectedChat=chatId){
    if(!selectedChat)return;
    try{
      const claims=await invoke('lu_research_claims',{chatId:selectedChat});
      setResearchClaimsByChat(old=>({...old,[selectedChat]:claims}));
    }catch(error){setResearchError(String(error));}
  }
  async function submitResearchClaim(){
    if(!chatId||researchReportBusy)return;
    const sourceChat=chatId;
    const {sourceId,claim,quote,stance,reasoning}=researchClaimDraft;
    if(!sourceId.trim()||!claim.trim()||!quote.trim()||!reasoning.trim())return;
    setResearchReportBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_add_claim',{
        chatId:sourceChat,sourceId:sourceId.trim(),claim:claim.trim(),
        quote:quote.trim(),stance,reasoning:reasoning.trim()
      });
      setResearchClaimsByChat(old=>({...old,[sourceChat]:result}));
    }catch(error){setResearchError(String(error));}
    finally{setResearchReportBusy(false);}
  }
  async function exportResearchReport(format){
    if(!chatId||researchReportBusy||!researchReportTitle.trim())return;
    const sourceChat=chatId;
    setResearchReportBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_export_report',{
        chatId:sourceChat,title:researchReportTitle.trim(),format
      });
      if(result)setResearchReportSavedByChat(old=>({...old,[sourceChat]:result}));
    }catch(error){setResearchError(String(error));}
    finally{setResearchReportBusy(false);}
  }
  async function refreshResearchCampaigns(selectedChat=chatId){
    if(!selectedChat)return;
    try{
      const result=await invoke('lu_research_campaigns',{chatId:selectedChat});
      setResearchCampaignsByChat(old=>({...old,[selectedChat]:result}));
    }catch(error){setResearchError(String(error));}
  }
  async function runResearchCampaign(){
    if(!chatId||researchCampaignBusy||!researchAvailable||!researchQuery.trim())return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      const result=await invoke('lu_research_campaign_start',{
        chatId:selectedChat,query:researchQuery.trim(),
        candidateCeiling:researchCampaignCeiling
      });
      setResearchCampaignsByChat(old=>({
        ...old,[selectedChat]:[result,...(old[selectedChat]||[])
          .filter(existing=>existing.campaignId!==result.campaignId)]
      }));
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function resumeResearchCampaign(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      await invoke('lu_research_campaign_resume',{chatId:selectedChat,campaignId});
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function cancelResearchCampaign(campaignId){
    if(!chatId||researchCampaignBusy)return;
    const selectedChat=chatId;
    setResearchCampaignBusy(true);setResearchError('');
    try{
      await invoke('lu_research_campaign_cancel',{chatId:selectedChat,campaignId});
    }catch(error){setResearchError(String(error));}
    finally{
      setResearchCampaignBusy(false);
      await refreshResearchCampaigns(selectedChat);
    }
  }
  async function retrieveResearchDocument(sourceId){
    if(!chatId||researchBusy||!researchAvailable)return;
    const selectedChat=chatId;
    setResearchBusy(true);setResearchError('');
    try{
      const document=await invoke('lu_research_retrieve_document',{
        chatId:selectedChat,sourceId
      });
      setResearchDocumentsByChat(old=>({
        ...old,[selectedChat+'|'+sourceId]:document
      }));
    }catch(error){setResearchError(String(error));}
    finally{setResearchBusy(false);}
  }
  async function verifyResearchDocumentPassage(sourceId){
    if(!chatId||researchBusy)return;
    const selectedChat=chatId;
    const key=selectedChat+'|'+sourceId;
    const quote=(researchDocumentQuotes[key]||'').trim();
    if(quote.length<4||quote.length>4096)return;
    setResearchBusy(true);setResearchError('');
    try{
      const proof=await invoke('lu_research_verify_document_passage',{
        chatId:selectedChat,sourceId,quote
      });
      setResearchDocumentPassages(old=>({...old,[key]:proof}));
    }catch(error){
      setResearchDocumentPassages(old=>({...old,[key]:null}));
      setResearchError(String(error));
    }finally{setResearchBusy(false);}
  }
  async function reviewBrowserEvidence(){
    if(!chatId||browserEvidenceBusy)return;
    const selectedChat=chatId;
    setBrowserEvidenceBusy(true);setResearchError('');
    try{
      const list=await invoke('lu_research_browser_evidence',{chatId:selectedChat});
      // Keep the host's read-only records scoped to the selected conversation.
      setBrowserEvidenceByChat(old=>({...old,[selectedChat]:list}));
    }catch(e){setResearchError(String(e));}
    finally{setBrowserEvidenceBusy(false);}
  }
  async function verifyObservedQuote(item){
    if(!chatId||researchQuoteBusy)return;
    const sourceChat=chatId;
    const key=sourceChat+'|'+item.generationId+'|'+item.callId;
    const quote=(researchQuoteDraftByKey[key]||'').trim();
    if(quote.length<4||quote.length>2048)return;
    setResearchQuoteBusy(true);setResearchError('');
    try{
      const publisherId=(researchQuoteSourceByKey[key]||'').trim();
      const result=await invoke('lu_research_verify_quote',{
        chatId:sourceChat,generationId:item.generationId,callId:item.callId,
        quote,publisherSourceId:publisherId||null
      });
      setResearchQuoteProofByKey(old=>({...old,[key]:result}));
    }catch(e){
      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
      setResearchError(String(e));
    }finally{setResearchQuoteBusy(false);}
  }
  async function inspectWithBrowserModel(){
    if(!chatId||!runtime.sessionId||!browserRuntime?.running||browserReadBusy)return;
    const selectedChat=chatId;
    setBrowserReadBusy(true);setBrowserReadError('');setBrowserReadResult(null);
    try{
      const result=await invoke('lu_browser_model_read',{
        sessionId:runtime.sessionId,chatId:selectedChat
      });
      setBrowserReadResult(result);
      setBrowserRuntime(await invoke('lu_browser_runtime_status'));
    }catch(error){
      setBrowserReadError(String(error));
      await invoke('lu_browser_runtime_status')
        .then(setBrowserRuntime).catch(()=>{});
    }finally{setBrowserReadBusy(false);}
  }
  async function browserOperation(name,args={}){
    if(browserBusy)return;
    setBrowserBusy(true);
    try{
      const result=await invoke(name,args);
      if(name==='lu_browser_stop')setBrowserRuntime(await invoke('lu_browser_runtime_status'));
      else setBrowserRuntime(result);
      setBrowserPreferenceError('');
    }catch(e){setBrowserPreferenceError(String(e));}
    finally{setBrowserBusy(false);}
  }
  useEffect(()=>{
    invoke('lu_observe_capture_available')
      .then(v=>setCaptureWorkerReady(Boolean(v)))
      .catch(()=>setCaptureWorkerReady(false));
  },[]);
  useEffect(()=>{
    setCaptureTaskId(null);
    if(!chatId){
      // Telling the native host that there is no active chat revokes any
      // Lu-UI observation lease rather than letting it run unseen.
      invoke('lu_observe_status',{chatId:''}).catch(()=>{});
      setObserveView({mode:'off'});return;
    }
    let active=true;
    invoke('lu_observe_status',{chatId})
      .then(v=>{if(active)setObserveView(v);})
      .catch(()=>{if(active)setObserveView({mode:'off'});});
    // An OS-owned clock/watchdog may expire consent while Lu is idle.
    const timer=setInterval(()=>{
      invoke('lu_observe_status',{chatId})
        .then(v=>{if(active)setObserveView(v);}).catch(()=>{});
    },5000);
    return()=>{active=false;clearInterval(timer);};
  },[chatId]);
  useEffect(()=>{
    if(!chatId||observeView.chatId!==chatId||observeView.mode==='off'){
      setObserveNotes([]);setObserveHandoff(null);return;
    }
    let active=true;
    invoke('lu_observe_notes',{chatId})
      .then(notes=>{if(active)setObserveNotes(notes);})
      .catch(()=>{if(active)setObserveNotes([]);});
    return()=>{active=false;};
  },[chatId,observeView.sessionId,observeView.mode,observeView.notesLogged]);
  useEffect(()=>{
    // Report only enum hints from Lu itself. No text, pixels, clipboard,
    // keystrokes, external app content, or native test PASS/FAIL.
    if(!chatId||observeView.mode!=='active'||observeView.chatId!==chatId)return;
    invoke('lu_observe_self_signal',{chatId,kind:'ui_changed'})
      .then(v=>setObserveView(v))
      .catch(()=>{});
  },[chatId,runtime.phase,observeView.mode]);
  useEffect(()=>{
    if(!chatId||!error||observeView.mode!=='active'||observeView.chatId!==chatId)return;
    invoke('lu_observe_self_signal',{chatId,kind:'error_indicator'})
      .then(v=>setObserveView(v)).catch(()=>{});
  },[chatId,error,observeView.mode]);
  async function loadImageRevisionHistory(){
    if(!chatId||imageHistoryBusy)return;
    const sourceChat=chatId;
    setImageHistoryBusy(true);
    try{
      const rows=await invoke('lu_image_workspace_history',{chatId:sourceChat});
      setImageHistoryView({chatId:sourceChat,rows:Array.isArray(rows)?rows:[]});
    }catch(e){setError(String(e));}
    finally{setImageHistoryBusy(false);}
  }
  async function exportImageRevision(){
    if(!chatId||!imageViewer?.imageRef||imageExportBusy)return;
    const region=['crop','redact','pixelate','rectangle'].includes(imageEdit.mode);
    const geometry=region||imageEdit.mode==='resize';
    const width=geometry?Number(imageEdit.width):imageViewer.width;
    const height=geometry?Number(imageEdit.height):imageViewer.height;
    const x=region?Number(imageEdit.x):0,y=region?Number(imageEdit.y):0;
    const blockSize=Number(imageEdit.blockSize);
    const stroke=Number(imageEdit.stroke);
    const hex=/^#[0-9a-fA-F]{6}$/.test(imageEdit.color||'')?
      parseInt(imageEdit.color.slice(1),16):null;
    if(!Number.isSafeInteger(width)||!Number.isSafeInteger(height)||
       width<1||height<1||width>8192||height>8192||width*height>32000000||
       (region&&(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||
         x<0||y<0||x+width>imageViewer.width||y+height>imageViewer.height))||
       (imageEdit.mode==='pixelate'&&(!Number.isSafeInteger(blockSize)||
         blockSize<2||blockSize>128))||
       (imageEdit.mode==='rectangle'&&(hex===null||!Number.isSafeInteger(stroke)||
         stroke<1||stroke>64))){
      setError('Choose valid dimensions, mask/annotation region and color.');return;
    }
    setImageExportBusy(true);setImageExportReceipt(null);
    try{
      const saved=await invoke('lu_image_workspace_export',{
        chatId,imageRef:imageViewer.imageRef,operation:imageEdit.mode,
        x:region?x:null,y:region?y:null,width,height,
        blockSize:imageEdit.mode==='pixelate'?blockSize:null,
        stroke:imageEdit.mode==='rectangle'?stroke:null,
        color:imageEdit.mode==='rectangle'?hex:null
      });
      if(saved){
        setImageExportReceipt(saved);
        await loadImageRevisionHistory();
      }
    }catch(e){setError(String(e));}
    finally{setImageExportBusy(false);}
  }
  async function observeCaptureOneShot(foregroundOnly=false){
    if(!chatId||!captureWorkerReady||observeView.mode!=='active'||captureStarting)return;
    setCaptureStarting(true);
    try{
      const id=await invoke('lu_observe_capture_one_shot',{chatId,foregroundOnly});
      if(id){setCaptureTaskId(id);setBackgroundPaneOpen(true);}
    }catch(e){setError(String(e));}
    finally{setCaptureStarting(false);}
  }
  async function reviewObserveHandoff(){
    if(!chatId||observeView.mode==='off')return;
    try{
      const handoff=await invoke('lu_observe_handoff',{chatId});
      setObserveHandoff(handoff);
    }catch(e){setError(String(e));}
  }
  function observeAction(action){
    if(!chatId)return;
    invoke('lu_observe_'+action,{chatId})
      .then(v=>{
        setObserveView(v);
        if(action==='pause'||action==='stop')setCaptureTaskId(null);
        setError('');
      })
      .catch(e=>setError(String(e)));
  }
  useEffect(()=>{
    if(!settingsOpen)return;
    let active=true;
    invoke('lu_browser_visibility')
      .then(mode=>{if(active){setBrowserVisibility(mode);setBrowserPreferenceError('');}})
      .catch(e=>{if(active)setBrowserPreferenceError(String(e));});
    return()=>{active=false;};
  },[settingsOpen]);

  async function refreshCatalog() {
    const [knownModels, status] = await Promise.all([invoke('lu_list_models'), invoke('lu_status')]);
    setModels(knownModels);
    setRuntime(status);
    setSelectedModel(old => old && knownModels.some(m => m.modelRef === old) ? old : status.modelRef || knownModels[0]?.modelRef || '');
  }
  async function startMediaFileScan() {
    setBackgroundError('');
    try {
      // Host-owned OS file dialog and pinned Lu worker, never model inference.
      const taskId=await invoke('lu_background_scan_file',{chatId:chatId||null});
      if (taskId) {
        setBackgroundFilter('active');
        await refreshBackgroundTasks();
      }
    } catch (error) {
      setBackgroundError(String(error));
    }
  }
  useEffect(()=>{
    let alive=true;
    if (!chatId){
      setAutoBackgroundContinuation(false);
      return ()=>{alive=false;};
    }
    invoke('lu_background_auto_continue',{chatId,enabled:null})
      .then(value=>{if(alive)setAutoBackgroundContinuation(Boolean(value));})
      .catch(error=>{if(alive)setBackgroundError(String(error));});
    return ()=>{alive=false;};
  },[chatId]);
  async function changeAutoBackgroundContinuation(checked){
    if (!chatId)return;
    try{
      const saved=await invoke('lu_background_auto_continue',{
        chatId,enabled:Boolean(checked)
      });
      setAutoBackgroundContinuation(Boolean(saved));
      setBackgroundError('');
    }catch(error){
      setBackgroundError(String(error));
    }
  }
  async function cancelBackgroundTask(taskId) {
    try {
      await invoke('lu_background_cancel', {taskId});
      await refreshBackgroundTasks();
    } catch (error) {
      // Native dialog denial and races against completion are not success.
      await refreshBackgroundTasks();
      if (!String(error).includes('Cancellation declined')) {
        setBackgroundError(String(error));
      }
    }
  }
  async function refreshBackgroundTasks() {
    try {
      if(!chatId){setBackgroundTasks([]);return;}
      const items=await invoke('lu_background_tasks',{chatId});
      setBackgroundTasks(Array.isArray(items)?items:[]);
      setBackgroundError('');
    } catch (error) {
      setBackgroundError(String(error));
    }
  }
  useEffect(()=>{
    if (!backgroundPaneOpen) return undefined;
    let alive=true;
    const refresh=()=>{if(alive) void refreshBackgroundTasks();};
    refresh();
    // UI polling queries Lu's local store, not the model or a worker.
    // Future host event pushes can replace this lightweight refresh.
    const timer=setInterval(refresh,2500);
    return ()=>{alive=false;clearInterval(timer);};
  },[backgroundPaneOpen,chatId]);

  async function refreshChats() {
    const list = await invoke('lu_chats');
    setChats(list.chats);
    setFolders(list.folders);
    return list.chats;
  }
  async function switchChat(id) {
    if (currentChat.current !== id) {
      // Capture the outgoing reading position before the asynchronous read.
      const el = transcriptRef.current;
      if (el) scrollByChat.current.set(currentChat.current || 'new', {
        top: el.scrollTop, follow: followOutput.current
      });
      pendingDrafts.current.set(currentChat.current || 'new', {
        prompt: draftText.current, images: draftImages.current, files: draftFiles.current
      });
    }
    requestedChat.current = id;
    const entries = await invoke('lu_history', {chatId: id});
    if (requestedChat.current !== id) return;
    currentChat.current = id;
    const draft = pendingDrafts.current.get(id) || {prompt:'',images:[],files:[]};
    draftText.current = draft.prompt;
    draftImages.current = draft.images;
    draftFiles.current = draft.files || [];
    setPrompt(draft.prompt);
    setImages(draft.images);
    setTextFiles(draft.files || []);
    setChatId(id);
    setHistory(entries);
    setError('');
  }
  useEffect(() => {
    let alive = true;
    let stopQueued = () => {}, stopStart = () => {}, stopToken = () => {}, stopReadProposal = () => {}, stopWriteProposal = () => {}, stopEditProposal = () => {}, stopNativeAction = () => {}, stopToolProgress = () => {}, stopTurnCapabilities = () => {}, stopBackground = () => {}, stopFinished = () => {}, stopBrowserChallenge = () => {};
    (async () => {
      try {
        const [list] = await Promise.all([refreshChats(), refreshCatalog()]);
        if (alive && list.length) await switchChat(list[0].chatId);
        if (alive) invoke('lu_document_settings')
          .then(value => {if (alive) setDocumentSettings(value);})
          .catch(error => {if (alive) setError(String(error));});
      } catch (error) { if (alive) setError(String(error)); }
    })();
    listen('lu-generation-queued', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => queuedJob(prev,e.payload));
    }).then(fn => { if (alive) stopQueued = fn; else fn(); });
    listen('lu-generation-start', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => startedJob(prev,e.payload));
    }).then(fn => { if (alive) stopStart = fn; else fn(); });
    listen('lu-browser-needs-human',e=>{
      if(!alive||!e.payload?.chatId)return;
      invoke('lu_browser_runtime_status')
        .then(v=>{if(alive)setBrowserRuntime(v);})
        .catch(()=>{});
    }).then(fn=>{if(alive)stopBrowserChallenge=fn;else fn();});
    listen('lu-token', e => {
      if (!alive || !e.payload?.chatId) return;
      setJobs(prev => tokenForJob(prev,e.payload));
    }).then(fn => { if (alive) stopToken = fn; else fn(); });
    listen('lu-read-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setReadProposals(prev => ({...prev, [proposal.proposalId]: proposal}));
    }).then(fn => { if (alive) stopReadProposal = fn; else fn(); });
    listen('lu-write-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setWriteProposals(prev => ({...prev, [proposal.proposalId]: proposal}));
    }).then(fn => { if (alive) stopWriteProposal = fn; else fn(); });
    listen('lu-edit-proposal', e => {
      const proposal = e.payload;
      if (!alive || !proposal?.chatId || !proposal?.proposalId) return;
      setEditProposals(prev => ({...prev, [proposal.proposalId]:proposal}));
    }).then(fn => { if (alive) stopEditProposal = fn; else fn(); });
    listen('lu-native-action-proposal',e=>{
      const proposal=e.payload;
      if (!alive||!proposal?.chatId||!proposal?.proposalId)return;
      setNativeActionProposals(prev=>({...prev,[proposal.proposalId]:proposal}));
    }).then(fn=>{if(alive)stopNativeAction=fn;else fn();});
    listen('lu-native-tool-progress',e=>{
      const update=e.payload;
      if (!alive||!update?.chatId||!update?.generationId)return;
      if (!['model_step','tool_started','tool_returned'].includes(update.phase))return;
      setToolProgressByChat(prev=>({
        ...prev,[update.chatId]:update,
      }));
    }).then(fn=>{if(alive)stopToolProgress=fn;else fn();});
    listen('lu-native-turn-capabilities',e=>{
      const entry=e.payload;
      if (!alive||!entry?.chatId||!entry?.generationId||!Array.isArray(entry.tools))return;
      setTurnCapabilitiesByChat(prev=>({...prev,[entry.chatId]:entry}));
    }).then(fn=>{if(alive)stopTurnCapabilities=fn;else fn();});
    listen('lu-generation-finish', e=>{
      if (!alive || !e.payload?.chatId)return;
      const {chatId:finishedChat,generationId}=e.payload;
      setToolProgressByChat(prev=>{
        if (prev[finishedChat]?.generationId!==generationId)return prev;
        const next={...prev};delete next[finishedChat];return next;
      });
      setTurnCapabilitiesByChat(prev=>{
        if (prev[finishedChat]?.generationId!==generationId)return prev;
        const next={...prev};delete next[finishedChat];return next;
      });
      setJobs(prev=>prev[finishedChat]?.generationId===generationId
        ?removeJob(prev,finishedChat):prev);
      // Native auto turns aren't initiated by a WebView invoke. Read their
      // journaled assistant output, including after a background tab switch.
      if (currentChat.current===finishedChat){
        invoke('lu_history',{chatId:finishedChat})
          .then(items=>{if(alive&&currentChat.current===finishedChat)setHistory(items);})
          .catch(()=>{});
      }
    }).then(fn=>{if(alive)stopFinished=fn;else fn();});
    listen('lu-background-terminal', e => {
      if (!alive || !e.payload?.taskId) return;
      // This event comes from the native host's persisted terminal ledger.
      // No AI generation or permission grant is performed by the WebView.
      setBackgroundTerminalNotice(e.payload);
      if (backgroundPaneOpen) void refreshBackgroundTasks();
    }).then(fn => { if (alive) stopBackground=fn; else fn(); });
    return () => { alive = false; stopQueued(); stopStart(); stopToken(); stopReadProposal(); stopWriteProposal(); stopEditProposal(); stopNativeAction(); stopToolProgress(); stopTurnCapabilities(); stopBackground(); stopFinished(); stopBrowserChallenge(); };
  }, []);
  // Follow new tokens only if the operator was already reading the end.
  // Remember independent scroll positions when switching conversations.
  function onTranscriptScroll() {
    const el = transcriptRef.current;
    if (!el) return;
    const follow = nearTranscriptBottom(el);
    followOutput.current = follow;
    scrollByChat.current.set(currentChat.current || 'new', {top: el.scrollTop, follow});
    setAtBottom(follow);
  }
  function jumpToLatest() {
    const el = transcriptRef.current;
    if (!el) return;
    followOutput.current = true;
    el.scrollTop = el.scrollHeight;
    scrollByChat.current.set(currentChat.current || 'new', {top: el.scrollTop, follow:true});
    setAtBottom(true);
  }
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el) return;
    const stored = scrollByChat.current.get(chatId || 'new');
    followOutput.current = stored?.follow ?? true;
    el.scrollTop = restoredScrollTop(stored, el.scrollHeight, el.clientHeight);
    setAtBottom(followOutput.current);
  }, [chatId]);
  useLayoutEffect(() => {
    const el = transcriptRef.current;
    if (!el || !followOutput.current) return;
    el.scrollTop = el.scrollHeight;
  }, [history, visibleStream, chatId]);
  useEffect(() => { if (!settingsOpen) composerRef.current?.focus(); }, [chatId]);
  useEffect(() => {
    function onShortcut(event) {
      if (event.defaultPrevented) return;
      if (isSearchKey(event) && !settingsOpen) {
        event.preventDefault(); setChatQuery(''); searchRef.current?.focus();
      } else if (isNewChatKey(event) && !settingsOpen) {
        event.preventDefault(); void newChat();
      } else if (event.key === 'Escape') {
        if (settingsOpen) setSettingsOpen(false);
        else if (modelMenuOpen) setModelMenuOpen(false);
        else if (document.activeElement === searchRef.current) {
          setChatQuery(''); composerRef.current?.focus();
        }
      }
    }
    document.addEventListener('keydown', onShortcut);
    return () => document.removeEventListener('keydown', onShortcut);
  }, [settingsOpen, modelMenuOpen]);
  useEffect(() => {
    if (!modelMenuOpen) return;
    function onOutside(event) {
      if (modelMenuRef.current && !modelMenuRef.current.contains(event.target))
        setModelMenuOpen(false);
    }
    document.addEventListener('pointerdown', onOutside);
    return () => document.removeEventListener('pointerdown', onOutside);
  }, [modelMenuOpen]);

  async function action(label, call) {
    setError(''); setActivity(label); setWorking(true);
    try { return await call(); }
    catch (error) { setError(String(error)); return undefined; }
    finally { setWorking(false); setActivity(''); }
  }
  async function newChat() {
    try {
      const created = await invoke('lu_new_chat');
      const list = await refreshChats();
      if (list.some(c => c.chatId === created.chatId)) await switchChat(created.chatId);
    } catch (error) { setError(String(error)); }
  }
  async function favoriteChat(chat) {
    await action('Updating favorite', async () => {
      await invoke('lu_favorite_chat', {chatId: chat.chatId, favorite: !chat.favorite});
      await refreshChats();
    });
  }
  async function moveChat(chat, folderId) {
    await action('Moving conversation', async () => {
      await invoke('lu_move_chat', {chatId: chat.chatId, folderId});
      await refreshChats();
    });
  }
  async function addFolder(e) {
    e.preventDefault();
    if (!folderName.trim()) return;
    await action('Creating folder', async () => {
      const created = await invoke('lu_new_folder', {name: folderName.trim()});
      setFolderDraft(false); setFolderName('');
      setExpandedFolders(p => [...p, created.folderId]);
      await refreshChats();
    });
  }
  function importModel() {
    action('Importing GGUF', async () => {
      const model = await invoke('lu_import_model');
      await refreshCatalog();
      if (model) setSelectedModel(model.modelRef);
    });
  }
  function attachProjector() {
    action('Attaching MMProj', async () => {
      await invoke('lu_attach_projector', {modelRef: selectedModel});
      await refreshCatalog();
    });
  }
  function rescanModels() {
    action('Scanning models folder', async () => {
      const count = await invoke('lu_rescan_models');
      await refreshCatalog();
      setActivity(count ? `Found ${count} new model${count === 1 ? '' : 's'}` : 'No new GGUF files');
    });
  }
  function loadModel() {
    if (!selectedModel || busy || loaded) return;
    setModelMenuOpen(false);
    action('Loading GGUF and projector', async () => {
      await invoke('lu_load_model', {modelRef: selectedModel});
      await refreshCatalog();
    });
  }
  function unloadModel() {
    action('Unloading model', async () => {
      await invoke('lu_unload_model', {sessionId: runtime.sessionId});
      await refreshCatalog();
    });
  }
  function pickImage() {
    action('Choosing image', async () => {
      const image = await invoke('lu_pick_image');
      if (image) setImages(prev => prev.some(p => p.imageRef === image.imageRef) ? prev : [...prev, image]);
    });
  }
  function removeImage(imageRef) {
    if (imageViewer?.imageRef===imageRef) setImageViewer(null);
    const image = draftImages.current.find(i => i.imageRef === imageRef);
    if (image?.previewUrl) { URL.revokeObjectURL(image.previewUrl); previewUrls.current.delete(image.previewUrl); }
    draftImages.current = draftImages.current.filter(i => i.imageRef !== imageRef);
    setImages(draftImages.current);
    invoke('lu_release_image', {imageRef}).catch(error => setError(String(error)));
  }
  function cancel() {
    const current = chatId && jobs[chatId];
    if (!current?.generationId || !runtime.sessionId || current.cancelling) return;
    const origin = chatId;
    setJobs(prev => ({...prev,[origin]:{...prev[origin],cancelling:true}}));
    invoke('lu_cancel', {sessionId: runtime.sessionId, generationId: current.generationId})
      .catch(error => {
        setJobs(prev => prev[origin] ? ({...prev,[origin]:{...prev[origin],cancelling:false}}) : prev);
        if (currentChat.current === origin) setError(String(error));
      });
    // Cancellation only targets this conversation's generation ID.
  }
  async function inspectSelectedTextFile() {
    if (working || chatBusy) return;
    let targetId = chatId;
    if (!targetId) {
      try {
        const newChat = await invoke('lu_new_chat');
        targetId = newChat.chatId;
        currentChat.current = targetId;
        setChatId(targetId);
        await refreshChats();
      } catch (error) { setError(String(error)); return; }
    }
    setError('');
    try {
      const result = await invoke('lu_inspect_text_file', {chatId: targetId});
      if (result && currentChat.current === targetId) {
        setHistory(await invoke('lu_history', {chatId: targetId}));
        setActivity(result.truncated ? 'File inspected (first 64 KiB)' : 'File inspected');
      }
      if (result) await refreshChats();
    } catch (error) {
      setError(String(error));
      await refreshChats().catch(() => {});
      if (currentChat.current === targetId)
        setHistory(await invoke('lu_history', {chatId: targetId}).catch(() => []));
    }
  }

  async function reviewReadProposal(proposal) {
    try {
      const result = await invoke('lu_approve_read_proposal', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      if (result) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await invoke('lu_history', {chatId: proposal.chatId}));
        setActivity(result.truncated ? 'Approved native read completed (truncated)' : 'Approved native read completed');
      }
    } catch (error) {
      setError(String(error));
    } finally {
      // The host consumes a proposal on selection, cancellation or rejection.
      setReadProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
    }
  }
  async function rejectReadProposal(proposal) {
    try {
      await invoke('lu_dismiss_read_proposal', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
    } catch (error) { setError(String(error)); }
    setReadProposals(prev => {
      const next = {...prev}; delete next[proposal.proposalId]; return next;
    });
  }

  async function refreshToolActivity(targetChat=chatId){
    if (!targetChat)return;
    setToolHistoryBusy(true);
    try{
      const entries=await invoke('lu_tool_turn_history',{chatId:targetChat});
      setToolHistoryByChat(prev=>({...prev,[targetChat]:entries||[]}));
    }catch(error){
      if (currentChat.current===targetChat)setError('Tool evidence unavailable: '+String(error));
    }finally{setToolHistoryBusy(false)}
  }
  async function refreshWriteRecovery(targetChat = chatId) {
    if (!targetChat) { setWriteRecovery([]); return; }
    const records = await invoke('lu_write_recovery', {chatId: targetChat});
    if (currentChat.current === targetChat) setWriteRecovery(records);
  }
  useEffect(() => {
    if (chatId) {
      refreshWriteRecovery(chatId).catch(() => setWriteRecovery([]));
      refreshEditRecovery(chatId).catch(() => setEditRecovery([]));
      refreshRollbackHistory(chatId).catch(() => setRollbackHistory([]));
      setEditObservations({}); setRollbackReviewed({});
    } else {
      setWriteRecovery([]); setEditRecovery([]);
      setEditObservations({}); setRollbackHistory([]); setRollbackReviewed({});
    }
  }, [chatId]);


  async function refreshEditRecovery(targetChat = chatId) {
    if (!targetChat) { setEditRecovery([]); return; }
    const records = await invoke('lu_edit_recovery',{chatId:targetChat});
    if (currentChat.current === targetChat) setEditRecovery(records);
  }
  async function refreshRollbackHistory(targetChat = chatId) {
    if (!targetChat) { setRollbackHistory([]); return; }
    const records = await invoke('lu_rollback_history',{chatId:targetChat});
    if (currentChat.current === targetChat) setRollbackHistory(records);
  }
  async function inspectEditRecovery(record) {
    try {
      const result = await invoke('lu_inspect_edit_recovery',{
        chatId,proposalId:record.proposalId
      });
      setEditObservations(prev => ({...prev,[record.proposalId]:result}));
      setRollbackReviewed({});
    } catch (error) { setError(String(error)); }
  }
  async function confirmRollback(record) {
    const observation = editObservations[record.proposalId];
    const reviewId = observation?.rollbackReviewId;
    if (!reviewId || !rollbackReviewed[reviewId]) return;
    setError('');
    try {
      const restored = await invoke('lu_confirm_rollback',{chatId,reviewId});
      if (restored) {
        setActivity('Restored previous version: '+restored.fileName);
        setLatestRestored({...restored,chatId});
        await refreshChats();
        if (currentChat.current === chatId)
          setHistory(await invoke('lu_history',{chatId}));
      } else setActivity('Kept current file; restoration denied');
    } catch (error) { setError(String(error)); }
    finally {
      setEditObservations(prev => {const next={...prev};delete next[record.proposalId];return next;});
      setRollbackReviewed(prev => ({...prev,[reviewId]:false}));
      await refreshRollbackHistory(chatId).catch(() => {});
      await refreshEditRecovery(chatId).catch(() => {});
    }
  }
  async function denyRollback(record) {
    const reviewId = editObservations[record.proposalId]?.rollbackReviewId;
    if (!reviewId) return;
    try {
      await invoke('lu_deny_rollback',{chatId,reviewId});
      setActivity('Kept current file; restoration not approved');
      setEditObservations(prev => {const next={...prev};delete next[record.proposalId];return next;});
      setRollbackReviewed(prev => ({...prev,[reviewId]:false}));
      await refreshRollbackHistory(chatId).catch(() => {});
    } catch (error) { setError(String(error)); }
  }
  async function reviewExistingFile(proposal) {
    setError('');
    try {
      const selected = await invoke('lu_review_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      setEditProposals(prev => ({
        ...prev,[proposal.proposalId]:{...prev[proposal.proposalId],selected}
      }));
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
    } catch (error) { setError(String(error)); }
  }
  async function approveExistingEdit(proposal) {
    if (!proposal.selected || !editReviewed[proposal.proposalId]) return;
    setError('');
    try {
      const edited = await invoke('lu_confirm_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      if (edited) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await invoke('lu_history',{chatId:proposal.chatId}));
        setActivity('Edited existing file: '+edited.fileName);
        setLatestEdited({...edited,proposalId:proposal.proposalId,chatId:proposal.chatId});
      } else setActivity('Edit denied in native confirmation');
    } catch (error) { setError(String(error)); }
    finally {
      setEditProposals(prev => {const next={...prev};delete next[proposal.proposalId];return next;});
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
      await refreshEditRecovery(proposal.chatId).catch(() => {});
    }
  }
  async function denyExistingEdit(proposal) {
    try {
      await invoke('lu_deny_edit_file',{
        chatId:proposal.chatId,proposalId:proposal.proposalId
      });
      setEditProposals(prev => {const next={...prev};delete next[proposal.proposalId];return next;});
      setEditReviewed(prev => ({...prev,[proposal.proposalId]:false}));
      await refreshEditRecovery(proposal.chatId).catch(() => {});
    } catch (error) { setError(String(error)); }
  }
  async function openEditedFolder(proposalId, targetChat = chatId) {
    try {
      await invoke('lu_open_edited_folder', {chatId:targetChat,proposalId});
    } catch (error) { setError(String(error)); }
  }
  async function openCreatedFolder(proposalId, targetChat = chatId) {
    try {
      await invoke('lu_open_created_folder', {chatId: targetChat, proposalId});
    } catch (error) { setError(String(error)); }
  }
  async function chooseDefaultDocumentsFolder() {
    try {
      const result = await invoke('lu_choose_default_document_folder');
      setDocumentSettings(result);
      setActivity('Default document folder updated');
    } catch (error) { setError(String(error)); }
  }
  async function resetDefaultDocumentsFolder() {
    try {
      const result = await invoke('lu_reset_default_document_folder');
      setDocumentSettings(result);
      setActivity('Default restored to system Documents');
    } catch (error) { setError(String(error)); }
  }
  async function rejectNativeActionProposal(proposal){
    try{
      await invoke('lu_deny_native_action_proposal',{
        chatId:proposal.chatId,proposalId:proposal.proposalId,
      });
      setNativeActionProposals(prev=>{
        const next={...prev};delete next[proposal.proposalId];return next;
      });
    }catch(error){setError(String(error));}
  }
  async function reviewNativeActionProposal(proposal){
    if (!chatId||proposal.chatId!==chatId||working||chatBusy)return;
    try{
      const approved=await invoke('lu_accept_native_action_proposal',{
        chatId:proposal.chatId,proposalId:proposal.proposalId,
      });
      setNativeActionProposals(prev=>{
        const next={...prev};delete next[proposal.proposalId];return next;
      });
      // This is NOT effect approval. Every branch opens an existing native
      // picker or a complete draft editor and requires an independent consent.
      switch(approved.action){
        case 'copy_file':await reviewNativeFileTransfer('copy');break;
        case 'move_file':await reviewNativeFileTransfer('move');break;
        case 'trash_file':await reviewNativeTrashFile();break;
        case 'extract_archive':await reviewArchiveTree();break;
        case 'create_document':setDocumentComposerOpen(true);break;
        case 'revise_office':setOfficeRevisionOpen(true);break;
        case 'batch_office':setOfficeBatchOpen(true);break;
        default:throw Error('Unrecognized native operation review');
      }
    }catch(error){setError(String(error));}
  }
  async function reviewArchiveTree() {
    if (!chatId || archiveBusy || working || chatBusy) return;
    setArchiveCancelRequested(false);
    setArchiveBusy(true);
    setError('');
    try {
      const review = await invoke('lu_review_archive_tree', {chatId});
      if (review) {
        setArchiveReview(review);
        setArchiveReviewed(false);
        setActivity('Archive review ready — inspect the complete file list before approval');
      }
    } catch (error) { setError(String(error)); }
    finally { setArchiveBusy(false); }
  }
  async function cancelArchivePreview() {
    if (!chatId || !archiveBusy || archiveCancelRequested) return;
    try {
      const requested=await invoke('lu_cancel_archive_preview',{chatId});
      if (requested) {
        setArchiveCancelRequested(true);
        setActivity('Stopping the supervised archive worker. No extraction approval was granted.');
      } else {
        setActivity('Archive picker or preview worker has already finished.');
      }
    } catch (error) {setError(String(error));}
  }
  async function confirmArchiveTree() {
    if (!archiveReview || !archiveReviewed || archiveBusy ||
        archiveReview.chatId !== chatId) return;
    setArchiveBusy(true);
    setArchiveCancelRequested(false);
    setError('');
    const review = archiveReview;
    try {
      const receipt = await invoke('lu_confirm_archive_tree', {
        chatId: review.chatId, reviewId: review.reviewId
      });
      if (receipt) {
        setLatestArchiveExtract({...receipt, chatId: review.chatId});
        setActivity('Archive extracted: ' + receipt.targetFolder);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await invoke('lu_history', {chatId: review.chatId}));
      } else {
        setActivity('Archive extraction denied in native confirmation');
      }
    } catch (error) {
      setError('Archive extraction was not confirmed. Do not retry an uncertain write: ' + String(error));
    } finally {
      // The host consumes a confirmed review once, even if extraction fails.
      // Never re-submit a possibly partially published archive operation.
      setArchiveReview(null);
      setArchiveReviewed(false);
      setArchiveBusy(false);
    }
  }
  async function requestArchiveCancellation() {
    if (!archiveReview || !archiveBusy || archiveCancelRequested) return;
    try {
      const signalled = await invoke('lu_cancel_archive_tree', {
        chatId: archiveReview.chatId, reviewId: archiveReview.reviewId
      });
      if (signalled) {
        setArchiveCancelRequested(true);
        setActivity('Cancellation requested. The native worker will stop at a safe checkpoint; inspect File operations for recovery.');
      } else {
        setActivity('The extraction worker has not started or has already finished. Use Cancel in the native confirmation dialog if it is open.');
      }
    } catch (error) { setError(String(error)); }
  }
  async function denyArchiveTree() {
    if (!archiveReview || archiveBusy) return;
    setArchiveBusy(true);
    try {
      await invoke('lu_deny_archive_tree', {
        chatId: archiveReview.chatId, reviewId: archiveReview.reviewId
      });
      setArchiveReview(null);
      setArchiveReviewed(false);
      setActivity('Archive extraction denied; no files created');
    } catch (error) { setError(String(error)); }
    finally { setArchiveBusy(false); }
  }

  async function reviewStructuredDocument() {
    if (!chatId || documentCreateBusy || !documentDraft.title.trim() ||
        !documentDraft.content.trim() || documentCreateReview) return;
    setDocumentCreateBusy(true);
    setError('');
    try {
      const view = await invoke('lu_review_document_create', {
        chatId,documentFormat:documentDraft.format,
        title:documentDraft.title,content:documentDraft.content
      });
      if (view) {
        setDocumentCreateReview(view);
        setDocumentCreateReviewed(false);
        setDocumentComposerOpen(false);
        setActivity('Document preview ready — review full content and target');
      }
    } catch (error) { setError(String(error)); }
    finally { setDocumentCreateBusy(false); }
  }
  async function confirmStructuredDocument() {
    if (!documentCreateReview || !documentCreateReviewed ||
        documentCreateBusy || documentCreateReview.chatId !== chatId) return;
    const review = documentCreateReview;
    setDocumentCreateBusy(true);
    setError('');
    try {
      const receipt = await invoke('lu_confirm_document_create', {
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestDocumentCreated({...receipt,chatId:review.chatId});
        setActivity('Created ' + receipt.documentFormat.toUpperCase() + ': ' + receipt.fileName);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await invoke('lu_history', {chatId:review.chatId}));
      } else {
        setActivity('Document creation denied in native confirmation');
      }
    } catch (error) {
      setError('Document creation not confirmed. Check recovery before retrying: ' + String(error));
    } finally {
      setDocumentCreateReview(null);
      setDocumentCreateReviewed(false);
      setDocumentCreateBusy(false);
    }
  }
  async function denyStructuredDocument() {
    if (!documentCreateReview || documentCreateBusy) return;
    setDocumentCreateBusy(true);
    try {
      await invoke('lu_deny_document_create', {
        chatId:documentCreateReview.chatId,
        reviewId:documentCreateReview.reviewId
      });
      setDocumentCreateReview(null);
      setDocumentCreateReviewed(false);
      setActivity('Document creation denied; no file was written');
    } catch (error) { setError(String(error)); }
    finally { setDocumentCreateBusy(false); }
  }

  async function reviewOfficeRevision() {
    if (!chatId || officeRevisionBusy || officeRevisionReview ||
        !officePatch.expectedOld.trim() || !officePatch.newValue.trim()) return;
    setOfficeRevisionBusy(true);
    setError('');
    try {
      const review=await invoke('lu_review_office_revision', {
        chatId,proposal:officePatch
      });
      if (review) {
        setOfficeRevisionReview(review);
        setOfficeRevisionReviewed(false);
        setOfficeRevisionOpen(false);
        setActivity('Office revision ready — review exact change and preserved package parts');
      }
    } catch (error) { setError(String(error)); }
    finally { setOfficeRevisionBusy(false); }
  }
  async function confirmOfficeRevision() {
    if (!officeRevisionReview || !officeRevisionReviewed ||
        officeRevisionBusy || officeRevisionReview.chatId !== chatId) return;
    const review=officeRevisionReview;
    setOfficeRevisionBusy(true);
    setError('');
    try {
      const receipt=await invoke('lu_confirm_office_revision', {
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestOfficeRevision({...receipt,chatId:review.chatId});
        setActivity('Saved new document revision: ' + receipt.targetNewName);
        await refreshChats();
        if (currentChat.current === review.chatId)
          setHistory(await invoke('lu_history', {chatId:review.chatId}));
      } else {
        setActivity('Office revision denied in native confirmation');
      }
    } catch (error) {
      setError('Office revision was not verified. Check recovery before retrying: ' + String(error));
    } finally {
      setOfficeRevisionReview(null);
      setOfficeRevisionReviewed(false);
      setOfficeRevisionBusy(false);
    }
  }
  async function denyOfficeRevision() {
    if (!officeRevisionReview || officeRevisionBusy) return;
    setOfficeRevisionBusy(true);
    try {
      await invoke('lu_deny_office_revision', {
        chatId:officeRevisionReview.chatId,
        reviewId:officeRevisionReview.reviewId
      });
      setOfficeRevisionReview(null);
      setOfficeRevisionReviewed(false);
      setActivity('Office revision denied; original file unchanged');
    } catch (error) { setError(String(error)); }
    finally { setOfficeRevisionBusy(false); }
  }

  async function reviewNativeFileTransfer(operation) {
    if (!chatId || fileTransferBusy || fileTransferReview || working || chatBusy) return;
    setFileTransferBusy(true);
    setError('');
    try {
      const view=await invoke('lu_review_file_transfer', {chatId,operation});
      if (view) {
        setFileTransferReview(view);
        setFileTransferReviewed(false);
        setActivity('Review the exact file content hash and action before approval');
      }
    } catch (error) { setError(String(error)); }
    finally { setFileTransferBusy(false); }
  }
  async function confirmNativeFileTransfer() {
    if (!fileTransferReview || !fileTransferReviewed || fileTransferBusy ||
        fileTransferReview.chatId !== chatId) return;
    const view=fileTransferReview;
    setFileTransferBusy(true);
    setError('');
    try {
      const receipt=await invoke('lu_confirm_file_transfer', {
        chatId:view.chatId,reviewId:view.reviewId
      });
      if (receipt) {
        setLatestFileTransfer({...receipt,chatId:view.chatId});
        setActivity('File ' + receipt.operation + ' completed: ' + receipt.targetName);
        await refreshChats();
        if (currentChat.current === view.chatId)
          setHistory(await invoke('lu_history', {chatId:view.chatId}));
      } else {
        setActivity('File operation denied in native confirmation');
      }
    } catch (error) {
      setError('File operation uncertain or rejected. Review before retrying: ' + String(error));
    } finally {
      setFileTransferReview(null);
      setFileTransferReviewed(false);
      setFileTransferBusy(false);
    }
  }
  async function denyNativeFileTransfer() {
    if (!fileTransferReview || fileTransferBusy) return;
    setFileTransferBusy(true);
    try {
      await invoke('lu_deny_file_transfer', {
        chatId:fileTransferReview.chatId,reviewId:fileTransferReview.reviewId
      });
      setFileTransferReview(null);
      setFileTransferReviewed(false);
      setActivity('File operation denied; original file preserved');
    } catch (error) { setError(String(error)); }
    finally { setFileTransferBusy(false); }
  }

  async function refreshTrashHistory(targetChat = chatId) {
    if (!targetChat) return;
    try {
      const records = await invoke('lu_file_trash_history', {chatId:targetChat});
      if (currentChat.current === targetChat) setTrashHistory(records || []);
    } catch (error) { setError('File custody could not be verified: ' + String(error)); }
  }
  async function reviewNativeTrashFile() {
    if (!chatId || trashBusy || trashReview || restoreReview || working || chatBusy) return;
    setTrashBusy(true); setError('');
    try {
      const proposal = await invoke('lu_review_file_trash', {chatId});
      if (proposal) {
        setTrashReview(proposal);
        setTrashReviewed(false);
        setTrashHistoryOpen(true);
      }
    } catch (error) { setError(String(error)); }
    finally { setTrashBusy(false); }
  }
  async function confirmNativeTrash() {
    const review = trashReview;
    if (!review || !trashReviewed || trashBusy || review.chatId!==chatId) return;
    setTrashBusy(true); setError('');
    try {
      const applied=await invoke('lu_confirm_file_trash',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (applied) {
        setLatestCustodyOutcome({...applied,chatId:review.chatId});
        setActivity('Reversible trash recorded: ' + applied.original);
        await refreshTrashHistory(review.chatId);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await invoke('lu_history',{chatId:review.chatId}));
      } else {
        setActivity('File trash denied in native confirmation');
      }
    } catch(error) {
      setError('Trash was rejected or its effect is uncertain. Inspect custody history; never retry blindly: '+String(error));
      await refreshTrashHistory(review.chatId);
    } finally {
      setTrashReview(null);
      setTrashReviewed(false);
      setTrashBusy(false);
    }
  }
  async function denyNativeTrash() {
    if (!trashReview || trashBusy) return;
    setTrashBusy(true);
    try {
      await invoke('lu_deny_file_trash',{
        chatId:trashReview.chatId,reviewId:trashReview.reviewId
      });
      setTrashReview(null);setTrashReviewed(false);
      setActivity('Reversible file operation denied; no files moved');
    } catch(error){setError(String(error));}
    finally {setTrashBusy(false);}
  }
  async function reviewRecordedRestore(custodyId) {
    if (!chatId || trashBusy || trashReview || restoreReview) return;
    setTrashBusy(true);setError('');
    try {
      const review=await invoke('lu_review_file_restore',{chatId,custodyId});
      setRestoreReview(review);
      setRestoreReviewed(false);
      setTrashHistoryOpen(true);
    } catch(error){setError('Cannot review restore: '+String(error));}
    finally{setTrashBusy(false);}
  }
  async function confirmRecordedRestore() {
    const review=restoreReview;
    if (!review || !restoreReviewed || trashBusy || review.chatId!==chatId) return;
    setTrashBusy(true);setError('');
    try {
      const result=await invoke('lu_confirm_file_restore',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (result) {
        setLatestCustodyOutcome({...result,chatId:review.chatId});
        setActivity('Verified original file restored: '+result.original);
        await refreshTrashHistory(review.chatId);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await invoke('lu_history',{chatId:review.chatId}));
      } else {
        setActivity('Restore denied in native confirmation');
      }
    } catch(error){
      setError('Restore was rejected or interrupted; inspect custody history before any further write: '+String(error));
      await refreshTrashHistory(review.chatId);
    } finally {
      setRestoreReview(null);
      setRestoreReviewed(false);
      setTrashBusy(false);
    }
  }
  async function denyRecordedRestore() {
    if (!restoreReview || trashBusy) return;
    setTrashBusy(true);
    try {
      await invoke('lu_deny_file_restore',{
        chatId:restoreReview.chatId,reviewId:restoreReview.reviewId
      });
      setRestoreReview(null);setRestoreReviewed(false);
      setActivity('Separate restoration denied');
    } catch(error){setError(String(error));}
    finally {setTrashBusy(false);}
  }

  async function refreshWave2Recovery(targetChat = chatId) {
    if (!targetChat || wave2RecoveryBusy) return;
    setWave2RecoveryBusy(true);
    try {
      const records=await invoke('lu_wave2_recovery',{chatId:targetChat});
      setWave2RecoveryByChat(prev=>({...prev,[targetChat]:records}));
    } catch (error) {
      setError('Cannot inspect Wave 2 operation journal: '+String(error));
    } finally {
      setWave2RecoveryBusy(false);
    }
  }

  function updateOfficeBatchField(index,key,value) {
    setOfficeBatchDraft(previous=>previous.map((patch,i)=>
      i===index ? {...patch,[key]:value} : patch));
  }
  async function reviewOfficeBatch() {
    if (!chatId || officeBatchBusy || officeBatchReview ||
        officeBatchDraft.length < 2 || officeBatchDraft.length > 32 ||
        officeBatchDraft.some(p=>!p.expectedOld.trim() || !p.newValue.trim())) return;
    setOfficeBatchBusy(true);setError('');
    try {
      const review=await invoke('lu_review_office_batch',{
        chatId,proposals:officeBatchDraft
      });
      if (review) {
        setOfficeBatchReview(review);
        setOfficeBatchReviewed(false);
        setOfficeBatchOpen(false);
        setActivity('Multi-edit Office package ready for complete review');
      }
    } catch(error){setError(String(error));}
    finally{setOfficeBatchBusy(false);}
  }
  async function confirmOfficeBatch() {
    const review=officeBatchReview;
    if (!review || !officeBatchReviewed || officeBatchBusy ||
        review.chatId !== chatId) return;
    setOfficeBatchBusy(true);setError('');
    try {
      const receipt=await invoke('lu_confirm_office_batch',{
        chatId:review.chatId,reviewId:review.reviewId
      });
      if (receipt) {
        setLatestOfficeBatch({...receipt,chatId:review.chatId});
        setActivity('Created approved multi-edit version: '+receipt.fileName);
        await refreshChats();
        if (currentChat.current===review.chatId)
          setHistory(await invoke('lu_history',{chatId:review.chatId}));
      }else{setActivity('Office batch denied by native confirmation');}
    }catch(error){
      setError('Office batch rejected or uncertain. Inspect recovery before retry: '+String(error));
    }finally{
      setOfficeBatchReview(null);setOfficeBatchReviewed(false);setOfficeBatchBusy(false);
    }
  }
  async function denyOfficeBatch() {
    if (!officeBatchReview || officeBatchBusy) return;
    setOfficeBatchBusy(true);
    try {
      await invoke('lu_deny_office_batch',{
        chatId:officeBatchReview.chatId,reviewId:officeBatchReview.reviewId
      });
      setOfficeBatchReview(null);setOfficeBatchReviewed(false);
      setActivity('Office batch denied; no revision created');
    }catch(error){setError(String(error));}
    finally{setOfficeBatchBusy(false);}
  }

  async function reviewWriteDestination(proposal, destinationMode = 'user_documents') {
    setError('');
    try {
      const destination = await invoke('lu_review_write_destination', {
        chatId: proposal.chatId, proposalId: proposal.proposalId, destinationMode
      });
      setWriteProposals(prev => ({
        ...prev,
        [proposal.proposalId]: {...prev[proposal.proposalId], destination}
      }));
    } catch (error) { setError(String(error)); }
  }
  async function confirmWrite(proposal) {
    setError('');
    try {
      const written = await invoke('lu_confirm_write', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      if (written) {
        await refreshChats();
        if (currentChat.current === proposal.chatId)
          setHistory(await invoke('lu_history', {chatId: proposal.chatId}));
        setActivity('Created new file: ' + written.fileName);
        setLatestCreated({...written, proposalId: proposal.proposalId, chatId: proposal.chatId});
      } else {
        setActivity('File creation denied in native confirmation');
      }
    } catch (error) { setError(String(error)); }
    finally {
      // The trusted host consumes the proposal exactly once, including on
      // failure. Never retry a create automatically.
      setWriteProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
      await refreshWriteRecovery(proposal.chatId).catch(() => {});
    }
  }
  async function denyWrite(proposal) {
    try {
      await invoke('lu_deny_write', {
        chatId: proposal.chatId, proposalId: proposal.proposalId
      });
      setWriteProposals(prev => {
        const next = {...prev}; delete next[proposal.proposalId]; return next;
      });
    } catch (error) { setError(String(error)); }
  }

  async function askReaderDocument() {
    // This deliberately opts in to a separate read-only structured model
    // tool turn. The native dialog supplies the document, not the WebView.
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    if (images.length || textFiles.length) {
      setError('Ask Reader selects one document with the native picker. Use normal Send for existing attachments.');
      return;
    }
    const input=prompt.trim();
    let activeId=chatId;
    if (!activeId) {
      const created=await invoke('lu_new_chat')
        .catch(error=>{setError(String(error));return null;});
      if (!created) return;
      activeId=created.chatId;
      currentChat.current=activeId;
      setChatId(activeId);
    }
    setJobs(prev=>({...prev,[activeId]:{
      generationId:null,phase:'selecting-document',position:0,text:''
    }}));
    setError('');
    try {
      const answer=await invoke('lu_reader_ask_document',{
        sessionId:runtime.sessionId,chatId:activeId,prompt:input
      });
      // A canceled OS picker returns null, without making a user turn.
      if (answer!==null && typeof answer!=='string') {
        throw Error('Reader returned an invalid model answer');
      }
      if (answer!==null && currentChat.current===activeId &&
          draftText.current===input) {
        draftText.current='';
        setPrompt('');
        const prior=pendingDrafts.current.get(activeId)||{};
        pendingDrafts.current.set(activeId,{...prior,prompt:''});
      }
    } catch(error) {
      if (currentChat.current===activeId &&
          !/Cancelled|Canceled/i.test(String(error))) {
        setError(String(error));
      }
    } finally {
      await refreshChats().catch(()=>{});
      if (currentChat.current===activeId){
        const entries=await invoke('lu_history',{chatId:activeId}).catch(()=>null);
        if (entries&&currentChat.current===activeId)setHistory(entries);
      }
      setJobs(prev=>removeJob(prev,activeId));
    }
  }
  async function askWorkspaceDirectory() {
    // Opt in to one native-selected read-only folder, never a WebView path.
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    if (images.length || textFiles.length) {
      setError('Ask Workspace selects a folder with the native picker. Use normal Send for existing attachments.');
      return;
    }
    const input=prompt.trim();
    let activeId=chatId;
    if (!activeId) {
      const created=await invoke('lu_new_chat')
        .catch(error=>{setError(String(error));return null;});
      if (!created) return;
      activeId=created.chatId;
      currentChat.current=activeId;
      setChatId(activeId);
    }
    setJobs(prev=>({...prev,[activeId]:{
      generationId:null,phase:'selecting-workspace',position:0,text:''
    }}));
    setError('');
    try {
      const answer=await invoke('lu_workspace_ask_directory',{
        sessionId:runtime.sessionId,chatId:activeId,prompt:input
      });
      if (answer!==null && typeof answer!=='string') {
        throw Error('Workspace returned an invalid model answer');
      }
      if (answer!==null && currentChat.current===activeId &&
          draftText.current===input) {
        draftText.current='';
        setPrompt('');
        const prior=pendingDrafts.current.get(activeId)||{};
        pendingDrafts.current.set(activeId,{...prior,prompt:''});
      }
    } catch(error) {
      if (currentChat.current===activeId &&
          !/Cancelled|Canceled/i.test(String(error))) {
        setError(String(error));
      }
    } finally {
      await refreshChats().catch(()=>{});
      if (currentChat.current===activeId) {
        const entries=await invoke('lu_history',{chatId:activeId}).catch(()=>null);
        if(entries&&currentChat.current===activeId)setHistory(entries);
      }
      setJobs(prev=>removeJob(prev,activeId));
    }
  }
  async function send(e) {
    e.preventDefault();
    if (working || chatBusy || !runtime.sessionId || !prompt.trim()) return;
    const input = prompt.trim();
    const useWorkspaceTools=workspaceToolsNextSend;
    const useBrowserResearch=browserResearchNextSend;
    if(useWorkspaceTools&&useBrowserResearch){
      setError('Choose either workspace tools or Browser research for this turn.');return;
    }
    if(useBrowserResearch&&(images.length||textFiles.length||input.length>12*1024)){
      setError('Browser research requires a text-only question of at most 12 KiB.');return;
    }
    if (useWorkspaceTools && (images.length || textFiles.length)){
      setError('Workspace tools require a text-only question. Remove attachments or turn off workspace tools.');return;
    }
    if (useWorkspaceTools && input.length>8192){
      setError('Workspace tool questions must be at most 8192 characters.');return;
    }
    let submitted;
    try { submitted = buildSubmittedPrompt(input, textFiles); }
    catch (error) {setError(String(error));return;}
    const submittedImages = [...images];
    const refs = submittedImages.map(i => i.imageRef);
    let activeId = chatId;
    if (!activeId) {
      const created = await invoke('lu_new_chat').catch(error => { setError(String(error)); return null; });
      if (!created) return;
      activeId = created.chatId;
      currentChat.current = activeId;
      setChatId(activeId);
    }
    // A workspace opt-in is consumed on this Send even if the picker is
    // denied/cancelled. No future conversation inherits a filesystem grant.
    if (useWorkspaceTools) setWorkspaceToolsNextSend(false);
    if (useBrowserResearch) setBrowserResearchNextSend(false);
    setImageViewer(null);
    // Capture the submitted turn immediately; leave the composer available
    // for drafting the next prompt while inference runs.
    draftText.current = '';
    draftImages.current = [];
    draftFiles.current = [];
    pendingDrafts.current.set(activeId, {prompt:'',images:[],files:[]});
    setPrompt(''); setImages([]); setTextFiles([]);
    if (currentChat.current === activeId) {
      setHistory(prev => [...prev, {role:'user',text:input,interrupted:false,cancelled:false}]);
    }
    setJobs(prev => ({...prev,[activeId]: {generationId:null,phase:'submitting',position:0,text:''}}));
    setError('');
    let completed = false;
    let cancelled = false;
    try {
      const output = useBrowserResearch
        ? await invoke('lu_generate_browser', {
            sessionId:runtime.sessionId,chatId:activeId,
            prompt:input,origins:browserOrigins,maxNewTokens:512
          })
        : await invoke('lu_generate', {
            sessionId: runtime.sessionId, chatId: activeId,
            prompt: submitted, imageRefs: refs, maxNewTokens: 512,
            readOnlyWorkspace: useWorkspaceTools
          });
      if (typeof output !== 'string') throw Error('Lu did not return a text response');
      completed = true;
    } catch (error) {
      cancelled = /Cancelled|Canceled/i.test(String(error));
      if (!cancelled && currentChat.current === activeId) setError(String(error));
    } finally {
      // Keep the streaming bubble until the journal has been read back.
      await refreshChats().catch(error => setError(String(error)));
      // A background turn must never steal focus from another conversation.
      if (currentChat.current === activeId) {
        const entries = await invoke('lu_history', {chatId:activeId})
          .catch(error => {setError(String(error));return null;});
        if (entries && currentChat.current === activeId) setHistory(entries);
      }
      setJobs(prev => removeJob(prev,activeId));
      await refreshCatalog().catch(error => { if (currentChat.current === activeId) setError(String(error)); });
    }
    if (!completed && !cancelled && currentChat.current === activeId && !draftText.current.trim()) {
      // Failed submissions remain editable. Already-journaled user turns are
      // still displayed; the next attempt can be revised by the operator.
      draftText.current = input;
      setPrompt(input);
    }
    for (const image of submittedImages) {
      if (image.previewUrl) {URL.revokeObjectURL(image.previewUrl);previewUrls.current.delete(image.previewUrl);}
      await invoke('lu_release_image', {imageRef:image.imageRef}).catch(() => {});
    }
  }
  async function attachReaderDocument() {
    // Keep the attachment in its originating chat even if the operator
    // switches conversations while the native system picker is open.
    const targetChat=currentChat.current||'new';
    try {
      const document=await invoke('lu_reader_pick_document');
      if (!document) return;
      const prior=pendingDrafts.current.get(targetChat);
      const currentlyVisible=targetChat===(currentChat.current||'new');
      const previous=prior?.files || (currentlyVisible?draftFiles.current:[]);
      if(previous.length>=MAX_TEXT_FILES){
        setError(`You can attach up to ${MAX_TEXT_FILES} documents per message`);
        return;
      }
      const extracted=document.truncated
        ?document.text+'\n[Reader: document text was truncated to fit the chat context]'
        :document.text;
      const item={
        name:document.name,
        size:document.bytesRead,
        text:extracted,
        format:document.format,
        truncated:document.truncated,
      };
      const next=[...previous,item];
      try{
        buildSubmittedPrompt(prior?.prompt ||
          (currentlyVisible?draftText.current:'Attachment') || 'Attachment',next);
      }catch(error){setError(String(error));return;}
      const old=prior||{
        prompt:currentlyVisible?draftText.current:'',
        images:currentlyVisible?draftImages.current:[],
        files:previous
      };
      pendingDrafts.current.set(targetChat,{...old,files:next});
      if(currentlyVisible){
        draftFiles.current=next;
        setTextFiles(next);
      }
      setError('');
    }catch(error){
      setError(String(error));
    }
  }
  async function importAttachments(files) {
    // Capture origin before a FileReader or native image admission can await.
    // A chat switch must not redirect an in-flight attachment to another draft.
    const targetChat = currentChat.current || 'new';
    function storeAttachment(kind, value) {
      const key=targetChat;
      const old=pendingDrafts.current.get(key) || {
        prompt: key === (currentChat.current||'new') ? draftText.current : '',
        images: key === (currentChat.current||'new') ? draftImages.current : [],
        files: key === (currentChat.current||'new') ? draftFiles.current : []
      };
      const next={...old,[kind]:value};
      pendingDrafts.current.set(key,next);
      if (key === (currentChat.current||'new')) {
        if(kind === 'images') {draftImages.current=value;setImages(value);}
        else {draftFiles.current=value;setTextFiles(value);}
      }
    }
    const items = Array.from(files || []).slice(0, 8);
    for (const file of items) {
      if (isPotentialImage(file)) {
        if (file.size <= 0 || file.size > MAX_IMAGE_ATTACHMENT) {
          setError(`Image ${file.name} exceeds the 8 MB limit`); continue;
        }
        if (!loadedModel?.projectorAttached) {
          setError('Load an MMProj-compatible model before attaching images'); continue;
        }
        try {
          const normalized = await normalizedImageFile(file);
          const dataUrl = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = () => reject(Error('Could not read image'));
            reader.readAsDataURL(normalized.blob);
          });
          const image = await invoke('lu_ingest_image_data_url', {dataUrl});
          if (image) {
            const previewUrl = URL.createObjectURL(file);
            const previous=pendingDrafts.current.get(targetChat)?.images ||
              (targetChat === (currentChat.current||'new') ? draftImages.current : []);
            if (previous.some(p => p.imageRef === image.imageRef)) URL.revokeObjectURL(previewUrl);
            else {
              previewUrls.current.add(previewUrl);
              storeAttachment('images',[...previous,{...image, name:file.name, size:file.size, previewUrl, detectedMimeType: normalized.mimeType}]);
            }
          }
        } catch (error) { setError(String(error)); }
      } else if (/\.(txt|md|csv|json|log|py|rs|js|jsx|ts|tsx)$/i.test(file.name)) {
        if (file.size <= 0 || file.size > MAX_TEXT_ATTACHMENT) {
          setError(`Text attachment ${file.name} exceeds the 64 KB limit`);continue;
        }
        try {
          const text = await file.text();
          const nextFile={name:file.name,size:file.size,text};
          const origin=pendingDrafts.current.get(targetChat);
          const previous=origin?.files || (targetChat === (currentChat.current||'new') ? draftFiles.current : []);
          const candidate=[...previous,nextFile];
          if(candidate.length>MAX_TEXT_FILES) {
            setError(`You can attach up to ${MAX_TEXT_FILES} text files per message`); continue;
          }
          try { buildSubmittedPrompt(origin?.prompt || draftText.current || 'Attachment',candidate); }
          catch(error) {setError(String(error));continue;}
          storeAttachment('files',candidate);
        } catch(error) { setError(String(error)); }
      } else {
        setError(`Unsupported attachment ${file.name}. Images and plaintext documents work; PDF/DOCX are not yet supported.`);
      }
    }
  }
  function handlePaste(event) {
    const fileItems = Array.from(event.clipboardData?.items || [])
      .filter(item => item.kind === 'file').map(item => item.getAsFile()).filter(Boolean);
    if (fileItems.length) { event.preventDefault(); void importAttachments(fileItems); }
  }
  function handleDrop(event) {
    event.preventDefault(); setDragActive(false);
    if (!loaded) { setError('Load a model before attaching files'); return; }
    void importAttachments(event.dataTransfer?.files);
  }
  function handleAppDrag(event) {
    if (Array.from(event.dataTransfer?.types || []).includes('Files')) {
      event.preventDefault();
      if (loaded) setDragActive(true);
    }
  }
  function beginComposerDrag(event) {
    event.preventDefault();
    dragSize.current = {y: event.clientY, height: composerHeight};
    const onMove = e => setComposerHeight(Math.max(100, Math.min(460, dragSize.current.height + dragSize.current.y - e.clientY)));
    const onUp = () => { window.removeEventListener('pointermove', onMove); window.removeEventListener('pointerup', onUp); dragSize.current = null; };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
  const selectModelText = loadedModel?.filename ?? selected?.filename ?? 'Choose a model';
  const renderChat = chat => <div key={chat.chatId} className={'chatRow ' + (chatId === chat.chatId ? 'active' : '')}>
    <button className="chatName" title={chat.title} onClick={() => switchChat(chat.chatId)}>{chat.title}</button>
    <button className={'favoriteAction ' + (chat.favorite ? 'isFavorite' : '')} title={chat.favorite ? 'Remove favorite' : 'Favorite'}
      onClick={() => favoriteChat(chat)} disabled={working} aria-label={chat.favorite ? 'Unfavorite chat' : 'Favorite chat'}>{chat.favorite ? '★' : '☆'}</button>
    <select className="chatMove" title="Move conversation to a folder" disabled={working} value={chat.folderId || ''}
      onChange={e => moveChat(chat,e.target.value || null)} aria-label={'Move ' + chat.title}>
      <option value="">—</option>{folders.map(f => <option key={f.folderId} value={f.folderId}>{f.name}</option>)}
    </select>
  </div>;
  return <div className={'shell' + (dragActive ? ' appDragActive' : '')}
    onDragEnter={handleAppDrag}
    onDragOver={handleAppDrag}
    onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget)) setDragActive(false); }}
    onDrop={handleDrop}>
    <aside className="sidebar">
      <div className="brand"><div className="brandMark">L</div><div><strong>Lu</strong><small>Independent desktop</small></div></div>
      <button className="newChatButton" onClick={newChat}><span>＋</span> New Chat</button>
      <div className="chatSearchRow"><span aria-hidden="true">⌕</span><input ref={searchRef} type="search"
        value={chatQuery} onChange={e => setChatQuery(e.target.value)} placeholder="Search chats"
        aria-label="Search conversations" title="Search conversations (Ctrl+K)"/></div>
      <div className="chatSidebarContent">
        {chatQuery.trim() ? <section className="chatSection"><h3>SEARCH RESULTS · {matchingChats.length}</h3>
          {matchingChats.map(renderChat)}
          {!matchingChats.length && <p className="asideHelp">No conversations match your search.</p>}
        </section> : <>
        {chats.some(c => c.favorite) && <section className="chatSection"><h3>FAVORITES</h3>
          {chats.filter(c => c.favorite).map(renderChat)}</section>}
        <section className="chatSection"><div className="sectionHeading"><h3>FOLDERS</h3>
          <button className="quietIcon" onClick={() => setFolderDraft(p => !p)} title="New folder" aria-label="New folder">+</button></div>
          {folderDraft && <form className="folderForm" onSubmit={addFolder}>
            <input autoFocus value={folderName} maxLength={100} placeholder="Folder name" onChange={e => setFolderName(e.target.value)}/>
            <button type="submit" disabled={!folderName.trim()}>Create</button></form>}
          {folders.map(folder => <div className="folderBlock" key={folder.folderId}>
            <button className="folderToggle" onClick={() => setExpandedFolders(p => p.includes(folder.folderId) ? p.filter(v => v !== folder.folderId) : [...p, folder.folderId])}>
              <span>{expandedFolders.includes(folder.folderId) ? '▾' : '▸'}</span> ▰ {folder.name} <small>{chats.filter(c => c.folderId === folder.folderId).length}</small></button>
            {expandedFolders.includes(folder.folderId) && <div className="folderChats">{chats.filter(c => c.folderId === folder.folderId).map(renderChat)}</div>}
          </div>)}
        </section>
        <section className="chatSection"><h3>RECENT CHATS</h3>{chats.filter(c => !c.folderId).map(renderChat)}
          {!chats.length && <p className="asideHelp">Your conversations will appear here. Models are never loaded automatically.</p>}
        </section></>}
      </div>
      <div className="sidebarFooter">
        <div className="runtimeStatus"><span className="statusDot" data-ready={loaded}></span><strong>{loaded ? 'Model loaded' : working && activity.startsWith('Loading') ? 'Loading model…' : 'No model loaded'}</strong></div>
        <small>{loaded ? loadedModel?.filename : 'Select a model from the chat header'}</small>
        <button className="settingsButton" onClick={() => setBackgroundPaneOpen(true)}>
          ◷ Background Tasks {activeBackgroundTasks.length ? '· '+activeBackgroundTasks.length : ''}
        </button>
        <button className="settingsButton" onClick={() => setSettingsOpen(true)}>⚙ Settings</button>
      </div>
    </aside>
    <main className="workspace">
      {backgroundTerminalNotice && <button type="button"
        className="backgroundGlobalNotice"
        onClick={()=>{setBackgroundPaneOpen(true);setBackgroundFilter('all');}}>
        <span>Background task ended: <strong>{backgroundTerminalNotice.state}</strong></span>
        <small>Open Tasks for details</small>
      </button>}
      <header className="topbar">
        <div className="chatTitle"><h1>{activeChat?.title && activeChat.title !== 'New chat' ? activeChat.title : 'Chat'}</h1>
          <p>{loaded ? 'Ready to chat · local model' : 'Choose a local GGUF to start'}</p></div>
        <div className="headerControls">
          {working && activity.startsWith('Loading') && <span className="busyHint" role="status" aria-live="polite"><span className="spinner" aria-hidden="true"></span> Loading model… {modelLoadSeconds}s elapsed · {lastHostLoadReply && Date.now() - lastHostLoadReply < 5000 ? 'Host responding' : 'Host response pending'}{modelLoadTrace?.loading && modelLoadTrace.events?.length > 0 ? ' · ' + modelLoadTrace.events[modelLoadTrace.events.length - 1].phase.replaceAll('_', ' ') : ''}</span>}
          <div className="modelSwitcherWrap" ref={modelMenuRef}>
            <button className="modelSwitcher" onClick={() => setModelMenuOpen(v => !v)} aria-expanded={modelMenuOpen} title="Quick model selection">
              <span className="modelText">{selectModelText}</span> <span>⌄</span></button>
            {modelMenuOpen && <div className="modelMenu">
              <div className="modelMenuHeading">LOCAL GGUF MODELS <small>Selection does not load</small></div>
              {models.map(m => <button className="quickModel" key={m.modelRef} onClick={() => { setSelectedModel(m.modelRef); setModelMenuOpen(false); }}>
                <span>{m.filename}<small>{m.projectorAttached ? 'GGUF + MMProj' : 'GGUF'}{!m.available ? ' · missing file' : ''}</small></span>
                {runtime.modelRef === m.modelRef && loaded ? <span className="readyMark">● Loaded</span> : selectedModel === m.modelRef ? '✓' : ''}
              </button>)}
              {!models.length && <div className="asideHelp">Put GGUF files in Lu's models directory or import them from Settings.</div>}
              <div className="modelMenuActions">
                <button className="primaryButton" disabled={busy || loaded || !selected?.available} onClick={loadModel}>Load selected</button>
                <button className="subtleButton" onClick={() => {setModelMenuOpen(false);setSettingsOpen(true);}}>Manage models</button>
              </div>
              {loaded && <button className="subtleButton full" disabled={busy} onClick={() => {setModelMenuOpen(false);unloadModel();}}>⏏ Unload active model</button>}
            </div>}
          </div>
          {loaded && <button className="quickEject" disabled={busy} title="Unload active model" onClick={unloadModel}>⏏</button>}
          <button className="quietSettings" title="Settings" onClick={() => setSettingsOpen(true)}>⚙</button>
        </div>
      </header>
      <div className="transcriptWrap"><div className="messages" ref={transcriptRef} onScroll={onTranscriptScroll} role="log" aria-live="polite" aria-relevant="additions">
        {!history.length && <div className="welcome"><div className="welcomeSymbol">L</div>
          <h2>What can I help you build?</h2>
          <p>Local conversations are saved separately. Start a new chat or choose one from your history.</p></div>}
        {displayedHistory.map((entry,i) => <div key={i} className={'message ' + (entry.role === 'user' ? 'user' : 'assistant')}>
          <span className="messageRole">{entry.role === 'user' ? 'You' : 'Lu'}{entry.cancelled ? ' · stopped' : entry.interrupted ? ' · interrupted' : ''}</span>
          <MessageText text={entry.text} role={entry.role}/></div>)}
        {visibleJob && <div className="message assistant streaming"><span className="messageRole">
          {visibleJob.phase === 'queued' ? `Lu · queued${visibleJob.position ? ' (#' + visibleJob.position + ')' : ''}`
           : visibleJob.phase === 'submitting' ? 'Lu · submitting'
           : visibleJob.cancelling ? 'Lu · stopping'
           : toolProgressByChat[chatId]?.generationId===visibleJob.generationId
             ? toolProgressByChat[chatId]?.phase==='tool_started'
               ? 'Lu · using approved native tool'
               : toolProgressByChat[chatId]?.phase==='tool_returned'
                 ? 'Lu · checking tool result'
                 : 'Lu · thinking with tools'
             : 'Lu · generating'}
          </span>{visibleStream ? <MessageText text={visibleStream}/> : <span className="spinner"/>}
          {turnCapabilitiesByChat[chatId]?.generationId===visibleJob.generationId &&
            <small>Read-only native tools for this turn: {turnCapabilitiesByChat[chatId].tools
              .map(tool=>tool.publicName).join(', ')}</small>}
        </div>}
      </div>
        {!atBottom && <button type="button" className="jumpLatest" onClick={jumpToLatest}
          aria-label="Jump to latest message">↓ Jump to latest</button>}
      </div>
      <div className="composerArea">
        <div className="composerResizer" onPointerDown={beginComposerDrag} role="separator"
          aria-label="Resize composer" aria-orientation="horizontal" tabIndex={0}
          aria-valuemin={100} aria-valuemax={460} aria-valuenow={composerHeight}
          onKeyDown={e => {if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
            e.preventDefault(); setComposerHeight(h => Math.max(100, Math.min(460, h + (e.key === 'ArrowUp' ? 20 : -20))));
          }}}>
          <span className="composerHandle">···</span>
          <button type="button" className="collapseComposer" onPointerDown={e => e.stopPropagation()} onClick={() => setComposerCollapsed(v => !v)}>{composerCollapsed ? 'Expand' : 'Collapse'} composer</button>
        </div>
        {chatId && <section className="observePilot" aria-label="Observe and Learn — Lu-only pilot">
          <div className="observePilotTop">
            <strong>Observe & Learn · Lu UI only</strong>
            <span role="status" className={'observeIndicator mode-'+observeView.mode}>
              {observeView.mode==='active'?'● Observing Lu UI events':
               observeView.mode==='paused'?'Paused':'Off'}
            </span>
          </div>
          <small>Lu UI event pilot: semantic state changes only · AI prompts issued: 0.
            An additionally approved Windows foreground-window capture rechecks HWND and PID;
            the general picker can select a monitor. Neither gives the model its pixels.</small>
          <div className="settingsActions">
            {observeView.mode==='off' &&
              <button type="button" onClick={()=>observeAction('start')}>Start observing Lu</button>}
            {observeView.mode==='active' &&
              <button type="button" onClick={()=>observeAction('pause')}>Pause</button>}
            {observeView.mode==='paused' &&
              <button type="button" onClick={()=>observeAction('resume')}>Resume</button>}
            {observeView.mode==='active' && captureWorkerReady &&
              <>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(true)}>
                  {captureStarting?'Capturing…':'Capture selected foreground app (4s)'}
                </button>
                <button type="button" disabled={captureStarting}
                  onClick={()=>observeCaptureOneShot(false)}>
                  Select window/display · capture one frame
                </button>
              </>}
            {observeView.mode!=='off' &&
              <button type="button" onClick={()=>observeAction('stop')}>Stop observation</button>}
          </div>
          {observeView.mode==='active' && !captureWorkerReady &&
            <small>Windows Graphics Capture is unavailable until the signed-off capture worker is installed with Lu.</small>}
          {captureTaskId && <small role="status">
            One-shot Windows frame capture task: {captureTaskId}.
            Check Background Tasks for the frame receipt. No image pixels are retained or sent to the model.
          </small>}
          {observeView.mode!=='off'&&
            <small>Observed state-change hints: {observeView.nativeEventCount||0}.
              Durable awareness notes: {observeView.notesLogged||0}.
              No automatically saved learning, GPU inference or GUI test verdict.</small>}
          {observeView.mode!=='off'&&
            <div className="observeNotations" aria-label="Local awareness notations">
              <strong>Awareness notes · native-only</strong>
              {observeNotes.length===0
                ? <small>No issues noted in this observation session.</small>
                : <ul>{observeNotes.slice(0,8).map(n=>
                  <li key={n.sequence}>
                    {n.kind==='error_indicator'?'Error indicator observed (not verified)':
                      n.kind==='verified_test_failure'?'Independently verified test failure':
                      n.kind} · {n.source==='renderer_hint'?'UI hint':n.source}
                    · {n.priority}
                  </li>
                )}</ul>}
              <button type="button" disabled={observeNotes.length===0}
                onClick={reviewObserveHandoff}>Prepare large-model review handoff</button>
              {observeHandoff&&<div role="status" className="observeHandoff">
                <strong>Main-model handoff prepared · not executed</strong>
                <small>Verified failures: {observeHandoff.verifiedFailures} ·
                  Unverified indicators: {observeHandoff.unverifiedIndicators}.
                  {observeHandoff.recommendation}.</small>
                <small>The main model has not run. Source changes, tests and
                  application control still require separate host authorization.</small>
              </div>}
            </div>}
        </section>}
        {wave2FileBusy && <section className="readProposalCard" role="status"
          aria-live="polite" aria-label="Native file operation in progress">
          <strong>Native file operation in progress</strong>
          <small>Lu is handling a native picker, confirmation, bounded document/archiving work,
            or receipt verification. Byte-level progress is not available in this workflow.
            No automatic retry occurs if a write is interrupted; inspect File operations or
            File recovery before taking another action.</small>
        </section>}
        {error && <div className="error" role="alert"><span>{error}</span><button onClick={() => setError('')}>Dismiss</button></div>}
        {Object.values(readProposals).filter(proposal => proposal.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard" role="group"
            aria-label="Model proposed file read">
            <strong>Lu requests a file read</strong>
            <p>{proposal.purpose}</p>
            <small>Requested filename: {proposal.fileName} · Up to {proposal.maxBytes} bytes.
              This request has no authorization until you select the matching file.</small>
            <div className="settingsActions">
              <button type="button" className="primaryButton" onClick={() => reviewReadProposal(proposal)}>
                Choose and approve file
              </button>
              <button type="button" className="subtleButton" onClick={() => rejectReadProposal(proposal)}>
                Deny
              </button>
            </div>
          </section>
        )}
        {Object.values(nativeActionProposals)
          .filter(proposal=>proposal.chatId===chatId).map(proposal=>
            <section key={proposal.proposalId} className="readProposalCard"
              role="group" aria-label="Lu suggested native action review">
              <strong>Review suggested action: {proposal.action.replaceAll('_',' ')}</strong>
              <p>{proposal.purpose}</p>
              <small>This model suggestion cannot change files. The native
                picker and a separate approval are required before execution.</small>
              <div className="settingsActions">
                <button type="button" className="primaryButton"
                  disabled={working||chatBusy}
                  onClick={()=>void reviewNativeActionProposal(proposal)}>
                  Review with native controls
                </button>
                <button type="button" className="subtleButton"
                  onClick={()=>void rejectNativeActionProposal(proposal)}>Deny</button>
              </div>
            </section>)}
        {toolActivityOpen && <section className="readProposalCard"
          role="region" aria-label="Read-only native tool activity">
          <strong>Native tool activity</strong>
          <small>Host-recorded Reader and Workspace tool-turn evidence. No
            permission is restored by inspecting this history; write effects
            remain in the separate File operations recovery panel.</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={toolHistoryBusy}
              onClick={()=>void refreshToolActivity(chatId)}>
              {toolHistoryBusy?'Refreshing…':'Refresh tool activity'}
            </button>
            <button type="button" className="subtleButton"
              onClick={()=>setToolActivityOpen(false)}>Close</button>
          </div>
          {(toolHistoryByChat[chatId]||[]).length===0&&
            <small>No persisted native model/tool events in this conversation.</small>}
          {(toolHistoryByChat[chatId]||[]).map(item=>
            <div key={item.source+':'+item.taskId} className="readProposalCard"
              role="group" aria-label={'Tool audit '+item.taskId}>
              <strong>{item.source} — {item.state.replaceAll('_',' ')}</strong>
              <small>{item.recordedAt} · Task {item.taskId}</small>
              <small>{item.modelResponses} model responses ·
                {item.toolFeedback} tool results ·
                {item.rejectedEvents} rejections ·
                {item.repairedResponses} structured repairs</small>
            </div>)}
        </section>}
        {(wave2RecoveryByChat[chatId] || []).length > 0 && <section
          className="readProposalCard" role="region" aria-label="Durable Wave 2 operation recovery">
          <strong>File and document operation history</strong>
          <small>Read-only records reloaded from Lu's durable host intent and effect journals.
            A matched receipt confirms a past journaled effect, not that no external process
            has subsequently changed the files.</small>
          {(wave2RecoveryByChat[chatId] || []).map(operation=>
            <div key={operation.effectCallId} className="readProposalCard">
              <strong>{operation.summary} — {operation.state.replaceAll('_',' ')}</strong>
              <code className="writeTarget">{operation.exactTarget}</code>
              <small>Tool: {operation.toolId}</small>
              <small>Approved SHA-256: {operation.approvedSha256}</small>
              <small>Effect ID: {operation.effectCallId}</small>
              <small>{operation.assessment}</small>
              {operation.targetIntegrity && operation.targetIntegrity !== 'NOT_RECHECKED' &&
                <small>Current output integrity: {operation.targetIntegrity}</small>}
              {!operation.receiptMatched &&
                <small>Do not automatically retry. Reconcile the filesystem and journal first.</small>}
            </div>
          )}
        </section>}
        {trashReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group" aria-label="Review reversible file trash">
          <strong>Move the EXACT selected file into Lu's reversible custody</strong>
          <small>Native-selected original file:</small>
          <code className="writeTarget">{trashReview.source}</code>
          <small>Lu-generated custody filename, in the same directory:</small>
          <code className="writeTarget">{trashReview.storedName}</code>
          <small>{trashReview.bytes} bytes · SHA-256 {trashReview.sha256}</small>
          <small>The original filename disappears on success. The same bytes are retained
            under a new Lu custody name in the same folder, NOT the system Recycle Bin.
            Restoration requires a separate human approval.</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={trashReviewed}
              onChange={e=>setTrashReviewed(e.target.checked)}/>
            I reviewed the exact file, checksum, custody filename, and reversible rename.
          </label>
          <small>An independent native confirmation still controls the write. Interrupted
            actions are never automatically retried.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!trashReviewed || trashBusy}
              onClick={()=>void confirmNativeTrash()}>Approve trash in native dialog</button>
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void denyNativeTrash()}>Deny trash</button>
          </div>
        </section>}
        {restoreReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group" aria-label="Review recorded file restoration">
          <strong>Restore a journal-verified file to its ORIGINAL filename</strong>
          <small>Custody ID: {restoreReview.custodyId}</small>
          <small>Exact restoration destination:</small>
          <code className="writeTarget">{restoreReview.original}</code>
          <small>Existing custody name: {restoreReview.trashEntry}</small>
          <small>{restoreReview.bytes} bytes · SHA-256 {restoreReview.sha256}</small>
          <small>Lu will never overwrite an existing file. Only a previously applied native
            custody receipt is eligible for restore. This requires a new one-use authorization.</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={restoreReviewed}
              onChange={e=>setRestoreReviewed(e.target.checked)}/>
            I reviewed the exact restoration destination, verified custody hash, and no-overwrite rule.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!restoreReviewed || trashBusy}
              onClick={()=>void confirmRecordedRestore()}>Approve restore in native dialog</button>
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void denyRecordedRestore()}>Deny restore</button>
          </div>
        </section>}
        {latestCustodyOutcome?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Reversible file operation: {latestCustodyOutcome.state}</strong>
          <small>Original filename: {latestCustodyOutcome.original}</small>
          <small>{latestCustodyOutcome.bytes} bytes · SHA-256 {latestCustodyOutcome.sha256}</small>
          <small>Custody ID: {latestCustodyOutcome.custodyId}</small>
          <small>Native effect receipt: {latestCustodyOutcome.effectCallId}</small>
        </section>}
        {trashHistoryOpen && chatId && <section className="readProposalCard"
          role="region" aria-label="Durable file custody and recovery">
          <strong>File custody and recovery</strong>
          <small>Read-only inspection of your journaled file trash and restore operations.
            Status is rechecked against stored receipts and the current filesystem.
            Uncertain operations require manual reconciliation; no automatic retry.</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton" disabled={trashBusy}
              onClick={()=>void refreshTrashHistory()}>Refresh custody status</button>
            <button type="button" className="subtleButton"
              onClick={()=>setTrashHistoryOpen(false)}>Close history</button>
          </div>
          {!trashHistory.length && <small>No durable file custody records for this chat.</small>}
          {trashHistory.map(item=><div key={item.custodyId} className="readProposalCard"
            role="group" aria-label={'Custody for '+item.original}>
            <strong>{item.original} — {item.state.replaceAll('_',' ')}</strong>
            <small>{item.bytes} bytes · SHA-256 {item.sha256}</small>
            <small>Observation: {item.observation.replaceAll('_',' ')}</small>
            <small>Trash receipt: {item.trashEffectId}</small>
            {item.restoreEffectId && <small>Restore receipt: {item.restoreEffectId}</small>}
            {item.restorable ?
              <button type="button" className="primaryButton"
                disabled={trashBusy || Boolean(restoreReview) || Boolean(trashReview)}
                onClick={()=>void reviewRecordedRestore(item.custodyId)}>
                Review restoration
              </button>
              : <small>{item.state==='UNCERTAIN' || item.observation.includes('NOT_')
                  ? 'No repeat or automatic recovery is authorized for this record.'
                  : 'Restore is not currently available for this record.'}</small>}
          </div>)}
        </section>}
        {fileTransferReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Review exact file copy or move">
          <strong>{fileTransferReview.operation==='move' ? 'MOVE existing file' : 'COPY existing file'}
            into a NEW filename</strong>
          <small>Native-picked source:</small>
          <code className="writeTarget">{fileTransferReview.sourceDisplay}</code>
          <strong>Host-generated new destination (never overwrites)</strong>
          <code className="writeTarget">{fileTransferReview.destinationDisplay}</code>
          <small>{fileTransferReview.bytes} bytes · source SHA-256: {fileTransferReview.sourceSha256}</small>
          <small>{fileTransferReview.removesOriginal
            ? 'MOVE removes the original name after linking the new one. A failed unlink requires manual reconciliation.'
            : 'COPY keeps the original file unchanged.'}</small>
          <label className="editReviewAck">
            <input type="checkbox" checked={fileTransferReviewed}
              onChange={e=>setFileTransferReviewed(e.target.checked)}/>
            I reviewed the source, destination, SHA-256 and {fileTransferReview.operation} consequences.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!fileTransferReviewed || fileTransferBusy}
              onClick={()=>void confirmNativeFileTransfer()}>Approve in native confirmation</button>
            <button type="button" className="subtleButton" disabled={fileTransferBusy}
              onClick={()=>void denyNativeFileTransfer()}>Deny operation</button>
          </div>
        </section>}
        {latestFileTransfer?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>File {latestFileTransfer.operation} complete: {latestFileTransfer.targetName}</strong>
          <code className="writeTarget">{latestFileTransfer.exactDestination}</code>
          <small>{latestFileTransfer.bytes} bytes · SHA-256: {latestFileTransfer.sha256}</small>
          <small>{latestFileTransfer.sourceRemoved
            ? 'Original filename removed.'
            : 'Original file retained.'} Effect receipt: {latestFileTransfer.effectCallId}</small>
        </section>}
        {officeBatchOpen && !officeBatchReview && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Prepare a multi-edit Office revision">
          <strong>Revise multiple Word, PowerPoint or Excel fields in one new version</strong>
          <small>Select the original document and a NEW output folder through native pickers.
            Every existing value must match. No modifications are made until final native approval.</small>
          {officeBatchDraft.map((patch,i)=><fieldset key={i} className="readProposalCard">
            <legend>Change {i+1} of {officeBatchDraft.length}</legend>
            <label className="editReviewAck">Kind
              <select value={patch.kind} onChange={e=>updateOfficeBatchField(i,'kind',e.target.value)}>
                <option value="word_text">Word text run</option>
                <option value="slide_text">PowerPoint slide text</option>
                <option value="cell_number">Excel numeric cell</option>
                <option value="cell_boolean">Excel boolean cell</option>
                <option value="cell_text">Excel inline text cell</option>
              </select>
            </label>
            {(patch.kind==='word_text'||patch.kind==='slide_text') &&
              <label className="editReviewAck">Text run index (zero based)
                <input type="number" min="0" max="9999" value={patch.textIndex}
                  onChange={e=>updateOfficeBatchField(i,'textIndex',
                    Math.max(0,Number(e.target.value)||0))}/>
              </label>}
            {patch.kind==='slide_text' && <label className="editReviewAck">Slide number
              <input type="number" min="1" max="256" value={patch.slideNumber}
                onChange={e=>updateOfficeBatchField(i,'slideNumber',
                  Math.max(1,Number(e.target.value)||1))}/>
            </label>}
            {patch.kind.startsWith('cell_') && <>
              <label className="editReviewAck">Worksheet number
                <input type="number" min="1" max="256" value={patch.sheetNumber}
                  onChange={e=>updateOfficeBatchField(i,'sheetNumber',
                    Math.max(1,Number(e.target.value)||1))}/>
              </label>
              <label className="editReviewAck">Cell address
                <input maxLength={8} value={patch.address}
                  onChange={e=>updateOfficeBatchField(i,'address',e.target.value.toUpperCase())}/>
              </label>
            </>}
            <label className="editReviewAck">Expected old value
              <textarea className="editFullText" rows={2} maxLength={32768}
                value={patch.expectedOld}
                onChange={e=>updateOfficeBatchField(i,'expectedOld',e.target.value)}/>
            </label>
            <label className="editReviewAck">New value
              <textarea className="editFullText" rows={2} maxLength={32768}
                value={patch.newValue}
                onChange={e=>updateOfficeBatchField(i,'newValue',e.target.value)}/>
            </label>
            <button type="button" className="subtleButton"
              disabled={officeBatchDraft.length<=2}
              onClick={()=>setOfficeBatchDraft(prev=>prev.filter((_,j)=>j!==i))}>
              Remove change
            </button>
          </fieldset>)}
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={officeBatchDraft.length>=32}
              onClick={()=>setOfficeBatchDraft(prev=>[...prev,{
                kind:prev[0]?.kind||'word_text',textIndex:prev.length,
                slideNumber:1,sheetNumber:1,address:'A1',expectedOld:'',newValue:''
              }])}>Add change</button>
            <button type="button" className="primaryButton"
              disabled={officeBatchBusy || officeBatchDraft.some(p=>!p.expectedOld.trim()||!p.newValue.trim())}
              onClick={()=>void reviewOfficeBatch()}>Native-pick source and review full batch</button>
            <button type="button" className="subtleButton"
              onClick={()=>setOfficeBatchOpen(false)}>Close</button>
          </div>
        </section>}
        {officeBatchReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Review all Office revision changes and unchanged package parts">
          <strong>Approve an exact {officeBatchReview.changes.length}-change Office version</strong>
          <small>Original document:</small>
          <code className="writeTarget">{officeBatchReview.sourceDisplay}</code>
          <small>New version destination (never overwritten):</small>
          <code className="writeTarget">{officeBatchReview.destinationDisplay}</code>
          <small>Original SHA-256: {officeBatchReview.originalSha256}</small>
          <small>Approved new package SHA-256: {officeBatchReview.revisedSha256}</small>
          <small>{officeBatchReview.changedParts} · {officeBatchReview.revisedBytes} bytes</small>
          <strong>All ordered exact changes</strong>
          {officeBatchReview.changes.map((change,i)=><div key={i} className="readProposalCard">
            <strong>{i+1}. {change.kind}: {change.target}</strong>
            <small>Before</small>
            <pre className="editFullText">{change.expectedOld}</pre>
            <small>After</small>
            <pre className="editFullText">{change.newValue}</pre>
          </div>)}
          <details open>
            <summary>Every unchanged package part ({officeBatchReview.unaffectedParts.length})</summary>
            <pre className="editFullText" aria-label="All unaffected Office part hashes">
              {officeBatchReview.unaffectedParts.map(p=>p[0]+' · '+p[1]).join('\n')}
            </pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={officeBatchReviewed}
              onChange={e=>setOfficeBatchReviewed(e.target.checked)}/>
            I reviewed every before/after change, exact file paths, and whole-package hashes.
          </label>
          <small>This batch is all-or-nothing before publication. The original remains unchanged.
            A separate native confirmation is required for this single-use write.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!officeBatchReviewed || officeBatchBusy}
              onClick={()=>void confirmOfficeBatch()}>Approve full batch in native dialog</button>
            <button type="button" className="subtleButton" disabled={officeBatchBusy}
              onClick={()=>void denyOfficeBatch()}>Deny all edits</button>
          </div>
        </section>}
        {latestOfficeBatch?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Office multi-edit version created: {latestOfficeBatch.fileName}</strong>
          <code className="writeTarget">{latestOfficeBatch.exactDestination}</code>
          <small>{latestOfficeBatch.bytesWritten} bytes · SHA-256: {latestOfficeBatch.sha256}</small>
          <small>Durable journal receipt: {latestOfficeBatch.effectCallId}</small>
        </section>}
        {officeRevisionOpen && !officeRevisionReview && <section
          className="readProposalCard editProposalCard" role="group"
          aria-label="Prepare an existing Office document revision">
          <strong>Revise existing Word, PowerPoint, or Excel document</strong>
          <small>The original file is selected with a native picker and never overwritten.
            A second native picker chooses the folder for a NEW versioned revision.</small>
          <label className="editReviewAck">Change type
            <select value={officePatch.kind}
              onChange={e=>setOfficePatch(prev=>({...prev,kind:e.target.value}))}>
              <option value="word_text">Word: text run</option>
              <option value="slide_text">PowerPoint: slide text run</option>
              <option value="cell_number">Excel: numeric cell</option>
              <option value="cell_boolean">Excel: boolean cell</option>
              <option value="cell_text">Excel: inline text cell</option>
            </select>
          </label>
          {(officePatch.kind==='word_text' || officePatch.kind==='slide_text') && <label className="editReviewAck">
            Text run index (0-based)
            <input type="number" min="0" max="9999" value={officePatch.textIndex}
              onChange={e=>setOfficePatch(prev=>({...prev,textIndex:Math.max(0,Number(e.target.value)||0)}))}/>
          </label>}
          {officePatch.kind==='slide_text' && <label className="editReviewAck">
            Slide number (1-based)
            <input type="number" min="1" max="256" value={officePatch.slideNumber}
              onChange={e=>setOfficePatch(prev=>({...prev,slideNumber:Math.max(1,Number(e.target.value)||1)}))}/>
          </label>}
          {officePatch.kind.startsWith('cell_') && <>
            <label className="editReviewAck">Worksheet number (1-based)
              <input type="number" min="1" max="256" value={officePatch.sheetNumber}
                onChange={e=>setOfficePatch(prev=>({...prev,sheetNumber:Math.max(1,Number(e.target.value)||1)}))}/>
            </label>
            <label className="editReviewAck">Cell address
              <input value={officePatch.address} maxLength={8}
                onChange={e=>setOfficePatch(prev=>({...prev,address:e.target.value.toUpperCase()}))}
                placeholder="B2"/>
            </label>
          </>}
          <label className="editReviewAck">Expected existing value (must match exactly)
            <textarea rows={2} className="editFullText" maxLength={32768}
              value={officePatch.expectedOld} onChange={e=>
                setOfficePatch(prev=>({...prev,expectedOld:e.target.value}))}/>
          </label>
          <label className="editReviewAck">New value
            <textarea rows={2} className="editFullText" maxLength={32768}
              value={officePatch.newValue} onChange={e=>
                setOfficePatch(prev=>({...prev,newValue:e.target.value}))}/>
          </label>
          <small>Unsupported formulas, signatures, macros, rich objects or unsafe structures are refused.
            Office revisions are new files; they never overwrite the selected original.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!chatId || officeRevisionBusy || !officePatch.expectedOld.trim() ||
                !officePatch.newValue.trim()}
              onClick={()=>void reviewOfficeRevision()}>Choose source & output folder</button>
            <button type="button" className="subtleButton"
              onClick={()=>setOfficeRevisionOpen(false)}>Close</button>
          </div>
        </section>}
        {officeRevisionReview?.chatId===chatId && <section
          className="readProposalCard editProposalCard"
          role="group" aria-label="Review exact Office document revision">
          <strong>New Office revision; original stays unchanged</strong>
          <small>Original source (SHA-256 {officeRevisionReview.originalSha256}):</small>
          <code className="writeTarget">{officeRevisionReview.sourceDisplay}</code>
          <small>New output (SHA-256 {officeRevisionReview.revisedSha256}):</small>
          <code className="writeTarget">{officeRevisionReview.destinationDisplay}</code>
          <small>Changed package part: {officeRevisionReview.targetPart}.
            Total revised document size: {officeRevisionReview.revisedBytes} bytes.</small>
          <strong>Exact proposed change</strong>
          <small>Before</small>
          <pre className="editFullText">{officeRevisionReview.originalValue}</pre>
          <small>After</small>
          <pre className="editFullText">{officeRevisionReview.newValue}</pre>
          <details open>
            <summary>All unchanged package parts ({officeRevisionReview.unaffectedParts.length})</summary>
            <pre className="editFullText" aria-label="Preserved Office package part checksums">
              {officeRevisionReview.unaffectedParts.map(x => x.partName + ' · ' + x.sha256).join('\n')}
            </pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={officeRevisionReviewed}
              onChange={e=>setOfficeRevisionReviewed(e.target.checked)}/>
            I reviewed the exact change, original and new paths, and all preserved package parts.
          </label>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!officeRevisionReviewed || officeRevisionBusy}
              onClick={()=>void confirmOfficeRevision()}>Approve new revision in native dialog</button>
            <button type="button" className="subtleButton" disabled={officeRevisionBusy}
              onClick={()=>void denyOfficeRevision()}>Deny revision</button>
          </div>
        </section>}
        {latestOfficeRevision?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Office document revision created: {latestOfficeRevision.targetNewName}</strong>
          <code className="writeTarget">{latestOfficeRevision.exactDestination}</code>
          <small>{latestOfficeRevision.bytesWritten} bytes · SHA-256 {latestOfficeRevision.sha256}</small>
          <small>Effect receipt: {latestOfficeRevision.effectCallId}. Original file preserved.</small>
        </section>}
        {documentComposerOpen && !documentCreateReview && <section
          className="readProposalCard editProposalCard" aria-label="Create structured Office document">
          <strong>Create a new structured Word or PowerPoint document</strong>
          <small>Lu will generate the file from these reviewed words, but only your native folder picker
            and independent native confirmation may authorize writing it.</small>
          <label className="editReviewAck">Document format
            <select value={documentDraft.format} onChange={e =>
              setDocumentDraft(prev=>({...prev,format:e.target.value}))}>
              <option value="docx">Word DOCX</option>
              <option value="pptx">PowerPoint PPTX</option>
            </select>
          </label>
          <label className="editReviewAck">Document title
            <input value={documentDraft.title} maxLength={256}
              onChange={e=>setDocumentDraft(prev=>({...prev,title:e.target.value}))}
              placeholder="Document or presentation title"/>
          </label>
          <label className="editReviewAck">Complete text
            <textarea className="editFullText" rows={8} maxLength={16384}
              value={documentDraft.content}
              onChange={e=>setDocumentDraft(prev=>({...prev,content:e.target.value}))}
              placeholder={documentDraft.format==='pptx'
                ? 'Bullet lines on slide one\n\nSlide two title\nBullet lines on slide two'
                : 'First paragraph\nSecond paragraph'}/>
          </label>
          <small>{documentDraft.format==='pptx'
            ? 'For presentations, blank lines divide slides. A single block uses the document title for slide one.'
            : 'For Word, each nonempty line becomes a paragraph after the title.'}</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton" disabled={documentCreateBusy ||
              !chatId || !documentDraft.title.trim() || !documentDraft.content.trim()}
              onClick={() => void reviewStructuredDocument()}>Choose output folder and review</button>
            <button type="button" className="subtleButton"
              onClick={() => setDocumentComposerOpen(false)}>Close</button>
          </div>
        </section>}
        {documentCreateReview?.chatId === chatId && <section className="readProposalCard editProposalCard"
          role="group" aria-label="Review complete structured document creation">
          <strong>New {documentCreateReview.documentFormat.toUpperCase()} document (no overwrite)</strong>
          <code className="writeTarget">{documentCreateReview.exactDestination}</code>
          <small>{documentCreateReview.bytes} bytes · {documentCreateReview.contentUnits} paragraphs/slides</small>
          <small>Exact package SHA-256: {documentCreateReview.sha256}</small>
          <strong>Title: {documentCreateReview.title}</strong>
          <small>Review ALL proposed content before confirming:</small>
          <pre className="editFullText" aria-label="All proposed document text">{documentCreateReview.content}</pre>
          <label className="editReviewAck">
            <input type="checkbox" checked={documentCreateReviewed}
              onChange={e => setDocumentCreateReviewed(e.target.checked)}/>
            I reviewed the entire new document content, exact file destination, and package hash.
          </label>
          <small>A separate native confirmation still grants the single-use write. Existing files
            are never replaced. Interrupted writes require recovery review.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!documentCreateReviewed || documentCreateBusy}
              onClick={() => void confirmStructuredDocument()}>Confirm in native dialog</button>
            <button type="button" className="subtleButton" disabled={documentCreateBusy}
              onClick={() => void denyStructuredDocument()}>Deny document creation</button>
          </div>
        </section>}
        {latestDocumentCreated?.chatId===chatId && <section className="readProposalCard" role="status">
          <strong>Created {latestDocumentCreated.documentFormat.toUpperCase()}:
            {latestDocumentCreated.fileName}</strong>
          <code className="writeTarget">{latestDocumentCreated.exactDestination}</code>
          <small>{latestDocumentCreated.bytesWritten} bytes · SHA-256: {latestDocumentCreated.sha256}</small>
          <small>Durable effect receipt: {latestDocumentCreated.effectCallId}</small>
        </section>}
        {archiveReview?.chatId === chatId && <section className="readProposalCard editProposalCard"
          role="group" aria-label="Review complete archive extraction">
          <strong>Extract archive into a NEW folder</strong>
          <small>Trusted OS file picker selected this archive; Lu has not extracted anything.</small>
          <code className="writeTarget">{archiveReview.sourceDisplay}</code>
          <strong>Exact destination (new folder only)</strong>
          <code className="writeTarget">{archiveReview.destinationDisplay}</code>
          <small>{archiveReview.files} files · {archiveReview.totalBytes} expanded bytes</small>
          <small>Entire tree manifest SHA-256: {archiveReview.manifestSha256}</small>
          <details open>
            <summary>Complete archive contents ({archiveReview.entries.length} entries)</summary>
            <pre className="editFullText" aria-label="All archive members">{archiveReview.entries.map((entry,i) =>
              (i + 1) + '. ' + entry.kind + ' · ' + entry.path +
              (entry.kind === 'file' ? ' · ' + entry.bytes + ' bytes · ' + entry.sha256 : '')
            ).join('\n')}</pre>
          </details>
          <label className="editReviewAck">
            <input type="checkbox" checked={archiveReviewed}
              onChange={e => setArchiveReviewed(e.target.checked)}/>
            I have reviewed the complete member list, destination, and no-overwrite behavior.
          </label>
          <small>Lu will ask again in a native confirmation dialog. If extraction is interrupted,
            partial files are retained for manual recovery and will never be silently retried.</small>
          <div className="settingsActions">
            <button type="button" className="primaryButton"
              disabled={!archiveReviewed || archiveBusy}
              onClick={() => void confirmArchiveTree()}>
              Approve in native confirmation
            </button>
            <button type="button" className="subtleButton" disabled={archiveBusy}
              onClick={() => void denyArchiveTree()}>Deny extraction</button>
            {archiveBusy && <button type="button" className="subtleButton"
              disabled={archiveCancelRequested}
              onClick={() => void requestArchiveCancellation()}>
              {archiveCancelRequested ? 'Cancellation requested' : 'Cancel running extraction'}
            </button>}
          </div>
        </section>}
        {latestArchiveExtract?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Archive extracted into {latestArchiveExtract.targetFolder}</strong>
          <code className="writeTarget">{latestArchiveExtract.exactDestination}</code>
          <small>{latestArchiveExtract.filesExtracted} verified files · {latestArchiveExtract.bytesExtracted} bytes</small>
          <small>Manifest SHA-256: {latestArchiveExtract.manifestSha256}</small>
          <small>Durable effect receipt: {latestArchiveExtract.effectCallId}</small>
        </section>}
        {Object.values(writeProposals).filter(p => p.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard writeProposalCard"
            role="group" aria-label="Review proposed new file">
            <strong>Lu proposes creating a NEW file (no overwrite)</strong>
            <p>{proposal.purpose}</p>
            <small>Filename: {proposal.fileName} · {proposal.bytes} bytes · SHA-256: {proposal.sha256}</small>
            <small>Model-selected text format: {proposal.fileFormat || 'UTF-8 text'}.
              JSON/JSONL receive syntax validation; source-code syntax needs its relevant parser or compiler.</small>
            <small>Review all proposed content before allowing creation:</small>
            <pre className="writePreview">{proposal.content}</pre>
            {proposal.destination ? <>
              <strong>Exact reviewed destination</strong>
              <code className="writeTarget">{proposal.destination.exactTarget}</code>
              <small>Only this destination and these exact bytes may be created.
                This approval is single-use and will not survive an interrupted attempt.</small>
            </> : <small>By default, save in your Documents folder. You can change the default under Settings → Advanced, or choose another folder for this file. Lu never chooses a destination without your approval.</small>}
            <div className="settingsActions">
              {!proposal.destination ? <>
                <button type="button" className="subtleButton"
                  onClick={() => reviewWriteDestination(proposal, 'user_documents')}>Use Documents folder</button>
                <button type="button" className="primaryButton"
                  onClick={() => reviewWriteDestination(proposal, 'choose_folder')}>Choose another folder</button>
              </> :
                <button type="button" className="primaryButton"
                  onClick={() => confirmWrite(proposal)}>Confirm create-new file</button>}
              <button type="button" className="subtleButton"
                onClick={() => denyWrite(proposal)}>Deny</button>
            </div>
          </section>
        )}

        {Object.values(editProposals).filter(p => p.chatId === chatId).map(proposal =>
          <section key={proposal.proposalId} className="readProposalCard editProposalCard"
            role="group" aria-label="Review proposed existing-file edit">
            <strong>Lu proposes editing an EXISTING text file</strong>
            <p>{proposal.purpose}</p>
            <small>Suggested filename: {proposal.fileName}. Lu cannot select the path;
              you must choose the matching existing file with a native picker.</small>
            {!proposal.selected ? <>
              <small>Proposed replacement (not approved):</small>
              <pre className="editFullText">{proposal.content}</pre>
            </> : <>
              <strong>Exact selected target</strong>
              <code className="writeTarget">{proposal.selected.exactTarget}</code>
              <small>Original: {proposal.selected.originalBytes} bytes, SHA-256: {proposal.selected.originalSha256}</small>
              <small>Replacement: {proposal.selected.replacementBytes} bytes, SHA-256: {proposal.selected.replacementSha256}</small>
              <small>Review fingerprint: {proposal.selected.approvalSha256} · Scope: {proposal.selected.risk}</small>
              <strong>Complete line-change view</strong>
              <small>Every unchanged line is retained; removed lines begin with −, added lines with +.
                Large change regions are not minimized. No content is truncated.</small>
              <pre className="editFullText editDiff" aria-label="Complete change lines">{fullEditChangeLines(proposal.selected.original,proposal.selected.replacement)}</pre>
              <strong>Complete original file</strong>
              <pre className="editFullText" aria-label="Complete original">{proposal.selected.original}</pre>
              <strong>Complete proposed replacement</strong>
              <pre className="editFullText" aria-label="Complete replacement">{proposal.selected.replacement}</pre>
              <label className="editReviewAck">
                <input type="checkbox" checked={Boolean(editReviewed[proposal.proposalId])}
                  onChange={e => setEditReviewed(prev => ({
                    ...prev,[proposal.proposalId]:e.target.checked
                  }))}/>
                I reviewed the complete original, changes, replacement and exact destination.
              </label>
              <small>Checking this box is only a UI acknowledgement; an independent native
                confirmation and host-authorized, journaled operation are still required.</small>
            </>}
            <div className="settingsActions">
              {!proposal.selected ?
                <button type="button" className="primaryButton"
                  onClick={() => reviewExistingFile(proposal)}>Choose existing file and compare</button> :
                <button type="button" className="primaryButton"
                  disabled={!editReviewed[proposal.proposalId]}
                  onClick={() => approveExistingEdit(proposal)}>Review native confirmation and edit</button>}
              <button type="button" className="subtleButton"
                onClick={() => denyExistingEdit(proposal)}>Deny edit</button>
            </div>
          </section>
        )}
        {latestEdited?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Edited {latestEdited.fileName}</strong>
          <small>Effect receipt: {latestEdited.effectCallId}</small>
          <small>Before: {latestEdited.originalSha256} · After: {latestEdited.replacementSha256}</small>
          <button className="folderLink" type="button"
            onClick={() => openEditedFolder(latestEdited.proposalId,latestEdited.chatId)}>
            Open containing folder
          </button>
        </section>}
        {latestCreated?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Created {latestCreated.fileName}</strong>
          <small>{latestCreated.exactTarget}</small>
          <button className="folderLink" type="button"
            onClick={() => openCreatedFolder(latestCreated.proposalId, latestCreated.chatId)}>
            Open containing folder
          </button>
        </section>}
        {writeRecovery.length > 0 && <section className="readProposalCard"
          aria-label="Write recovery records">
          <strong>Write recovery and audit</strong>
          <small>Read-only evidence from durable approval and effect journals.
            Uncertain operations are not automatically retried.</small>
          {writeRecovery.map(record =>
            <div key={record.proposalId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Approval: {record.approval} · Effect: {record.effect} · {record.bytes} bytes</small>
              <small>Observed target: {record.observedTarget} (observation only; not execution proof)</small>
              <small>Reconciliation: {record.assessment} — no automatic retry or reauthorization</small>
              {record.effect === 'Applied' && record.approval === 'Approved' &&
                <button type="button" className="folderLink"
                  onClick={() => openCreatedFolder(record.proposalId, chatId)}>
                  Open containing folder
                </button>}
              {record.exactTarget && <code className="writeTarget">{record.exactTarget}</code>}
              <small>SHA-256: {record.sha256}</small>
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshWriteRecovery().catch(error => setError(String(error)))}>
            Refresh recovery evidence
          </button>
        </section>}

        {editRecovery.length > 0 && <section className="readProposalCard"
          aria-label="Existing file edit recovery records">
          <strong>Existing-file edit recovery</strong>
          <small>Read-only approval and effect journal evidence. File bytes are inspected only
            after you reselect the originally approved containing folder through the native picker.
            Uncertain edits never resume automatically.</small>
          {editRecovery.map(record =>
            <div key={record.proposalId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Review: {record.approval} · Effect: {record.effect}</small>
              <small>Exact applied edit receipt: {record.receiptVerified ? 'Verified' : 'Not verified'}</small>
              {record.exactTarget && <code className="writeTarget">{record.exactTarget}</code>}
              <small>Original SHA-256: {record.originalSha256 || 'Not selected'}</small>
              <small>Replacement SHA-256: {record.replacementSha256}</small>
              <small>Assessment: {(editObservations[record.proposalId]?.assessment || record.assessment)}</small>
              <small>Observed bytes: {(editObservations[record.proposalId]?.observation || record.observation)}
                (matching bytes alone do not prove who published them)</small>
              {editObservations[record.proposalId] && <small>
                Undo preflight: {editObservations[record.proposalId].rollbackReviewStatus}
              </small>}
              {editObservations[record.proposalId]?.rollbackPreview && <details
                className="editUndoPreview" aria-label="Read-only undo preview">
                <summary>Review verified original for a possible future restore (read-only)</summary>
                <small>No restore command is issued. This preview requires the exact
                  decoded APPLIED edit receipt, retained original backup and current file.</small>
                <small>Edited SHA-256: {editObservations[record.proposalId].rollbackPreview.replacementSha256}</small>
                <small>Original SHA-256: {editObservations[record.proposalId].rollbackPreview.originalSha256}</small>
                <strong>Complete changes if original were restored</strong>
                <pre className="editFullText editDiff" aria-label="Proposed restore change lines">{
                  fullEditChangeLines(editObservations[record.proposalId].rollbackPreview.currentText,
                    editObservations[record.proposalId].rollbackPreview.restoreText)
                }</pre>
                <strong>Complete current text</strong>
                <pre className="editFullText" aria-label="Current text for possible restore">{
                  editObservations[record.proposalId].rollbackPreview.currentText
                }</pre>
                <strong>Complete verified original text</strong>
                <pre className="editFullText" aria-label="Verified original text">{
                  editObservations[record.proposalId].rollbackPreview.restoreText
                }</pre>
                <small>This restoration has its own authorization. A native confirmation
                  will ask again before any filesystem operation. The edited version is separately
                  backed up. Changes made by another program will block the restore.</small>
                {editObservations[record.proposalId].rollbackReviewId && <>
                  <label className="editReviewAck">
                    <input type="checkbox"
                      checked={Boolean(rollbackReviewed[editObservations[record.proposalId].rollbackReviewId])}
                      onChange={e => setRollbackReviewed(prev => ({
                        ...prev,
                        [editObservations[record.proposalId].rollbackReviewId]:e.target.checked
                      }))}/>
                    I reviewed the complete current file, original version and restoration changes.
                  </label>
                  <div className="settingsActions">
                    <button className="primaryButton" type="button"
                      disabled={!rollbackReviewed[editObservations[record.proposalId].rollbackReviewId]}
                      onClick={() => confirmRollback(record)}>
                      Confirm original restore in native dialog
                    </button>
                    <button className="subtleButton" type="button"
                      onClick={() => denyRollback(record)}>Keep current version</button>
                  </div>
                </>}
              </details>}
              {record.approval === 'Approved' && <button type="button" className="subtleButton"
                onClick={() => inspectEditRecovery(record)}>Reselect folder to inspect artifacts</button>}
              {record.approval === 'Approved' && record.receiptVerified &&
                <button type="button" className="folderLink"
                  onClick={() => openEditedFolder(record.proposalId,chatId)}>
                  Open containing folder
                </button>}
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshEditRecovery().catch(error => setError(String(error)))}>
            Refresh edit recovery ledger
          </button>
        </section>}
        {rollbackHistory.length > 0 && <section className="readProposalCard"
          aria-label="Restoration authorization and recovery history">
          <strong>Restoration history and recovery</strong>
          <small>Separate user approval, journal effect, and verified receipt. A failed or
            uncertain restore is never automatically repeated, even after a restart.</small>
          {rollbackHistory.map(record =>
            <div key={record.reviewId} className="writeRecoveryRow">
              <strong>{record.fileName}</strong>
              <small>Review: {record.decision} · Restore effect: {record.effect}</small>
              <small>Verified restoration receipt: {record.receiptVerified ? 'Yes' : 'No'}</small>
              <small>Assessment: {record.assessment}</small>
              <small>Original edit effect: {record.originalEditId}</small>
              <small>Restore SHA-256: {record.restoredSha256}</small>
              <code className="writeTarget">{record.exactTarget}</code>
            </div>
          )}
          <button type="button" className="subtleButton"
            onClick={() => refreshRollbackHistory().catch(error => setError(String(error)))}>
            Refresh restoration history
          </button>
        </section>}
        {latestRestored?.chatId === chatId && <section className="readProposalCard" role="status">
          <strong>Original version restored: {latestRestored.fileName}</strong>
          <small>Restored {latestRestored.restoredBytes} bytes · SHA-256: {latestRestored.restoredSha256}</small>
          <small>New rollback effect: {latestRestored.rollbackEffectId}</small>
          <small>The prior edited bytes were retained as a separate rollback backup.</small>
        </section>}
        {!composerCollapsed && <form className={'composer' + (dragActive ? ' dragActive' : '')} onSubmit={send}>
          {(images.length > 0 || textFiles.length > 0) && <div className="attachments" aria-label="Draft attachments">
            {images.map(im => <span key={im.imageRef} className="attachment imageAttachment">
              {im.previewUrl ? <img src={im.previewUrl} alt="Attached image preview"/> : <span aria-hidden="true">▧</span>}
              <span title={im.name || im.mimeType}>{im.name || im.mimeType}{im.detectedMimeType && im.name?.toLowerCase().endsWith('.jpg') && im.detectedMimeType === 'image/png' ? ' (PNG data)' : ''}</span>
              {im.previewUrl && <button type="button" onClick={()=>setImageViewer({imageRef:im.imageRef,zoom:1})} aria-label={'View image '+(im.name||'draft')}>View</button>}
              <button type="button" onClick={() => removeImage(im.imageRef)} aria-label="Remove image">×</button>
            </span>)}
            {textFiles.map((file,i)=><span key={i} className="attachment textAttachment"><span aria-hidden="true">▤</span>
              <span title={attachmentSummary(file)}>{attachmentSummary(file)}</span>
              <button type="button" onClick={() => {const next=draftFiles.current.filter((_,n)=>n!==i);draftFiles.current=next;setTextFiles(next);}} aria-label={'Remove '+file.name}>×</button>
            </span>)}
          </div>}
          <textarea ref={composerRef} aria-label="Message Lu" value={prompt} onChange={e => {draftText.current=e.target.value;setPrompt(e.target.value);}} style={{height:composerHeight}} maxLength={16384}
            placeholder={loaded ? 'Message Lu… Paste or drop images and text files' : 'Load a GGUF model first…'} disabled={!loaded}
            onPaste={handlePaste}
            onKeyDown={e => { if (isSendKey(e)) { e.preventDefault(); e.currentTarget.form?.requestSubmit(); } }}/>
          <div className="composerFooter"><input ref={fileInput} type="file" className="visuallyHidden" multiple
              accept=".png,.jpg,.jpeg,.txt,.md,.csv,.json,.log,.py,.rs,.js,.jsx,.ts,.tsx" onChange={e => {void importAttachments(e.target.files);e.target.value='';}}/>
            <button type="button" className="subtleButton" disabled={!loaded} onClick={() => fileInput.current?.click()}>+ Attach</button>
            <button type="button" className="subtleButton" disabled={!loaded}
              title="Inspect native vision status without claiming MMProj presence proves visual understanding"
              onClick={()=>void invoke('lu_vision_capability')
                .then(setVisionCapability).catch(e=>setError(String(e)))}>Check vision</button>
            {visionCapability?.sessionId===runtime.sessionId && visionCapability?.modelRef===runtime.modelRef &&
              <span className="composerHelp" role="status">
                Vision: {visionCapability.status} · {visionCapability.detail}
              </span>}
            <button type="button" className="subtleButton"
              disabled={!chatId || wave2RecoveryBusy}
              title="Read-only durable history of approved archives, documents and file operations"
              onClick={()=>void refreshWave2Recovery(chatId)}>
              {wave2RecoveryBusy ? 'Checking operations…' : 'File operations'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || trashBusy || Boolean(trashReview) || Boolean(restoreReview)}
              title="Native-pick one file and review a reversible same-directory custody operation"
              onClick={()=>void reviewNativeTrashFile()}>Trash file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || trashBusy} title="Inspect journaled trash/restore receipts, including interrupted operations"
              onClick={()=>{setTrashHistoryOpen(v=>!v);void refreshTrashHistory();}}>
              File recovery
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || fileTransferBusy || Boolean(fileTransferReview)}
              title="Native-pick an existing file, review exact SHA-256 and approve a no-clobber copy"
              onClick={()=>void reviewNativeFileTransfer('copy')}>Copy file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || fileTransferBusy || Boolean(fileTransferReview)}
              title="Native-pick an existing file and approve a same-directory move into a new filename"
              onClick={()=>void reviewNativeFileTransfer('move')}>Move file</button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || officeBatchBusy || Boolean(officeBatchReview)}
              title="Review up to 32 Word, PowerPoint or Excel changes, then create one new version"
              onClick={()=>setOfficeBatchOpen(v=>!v)}>
              {officeBatchOpen ? 'Close batch' : 'Batch Office'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || officeRevisionBusy || Boolean(officeRevisionReview)}
              title="Revise one existing Office field while preserving original and unrelated package contents"
              onClick={()=>setOfficeRevisionOpen(v=>!v)}>
              {officeRevisionOpen ? 'Close revision' : 'Revise Office'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || documentCreateBusy || Boolean(documentCreateReview)}
              title="Create a new DOCX or PPTX through a native folder picker and exact-content approval"
              onClick={() => setDocumentComposerOpen(v=>!v)}>
              {documentComposerOpen ? 'Close document' : '+ Document'}
            </button>
            <button type="button" className="subtleButton"
              disabled={!chatId || working || chatBusy || archiveBusy || Boolean(archiveReview)}
              title="Use native file and folder pickers to safely review an archive and extract it into one NEW folder"
              onClick={() => void reviewArchiveTree()}>{archiveBusy ? 'Archive…' : 'Extract archive'}</button>
            {archiveBusy && !archiveReview && <button type="button"
              className="subtleButton" disabled={archiveCancelRequested}
              onClick={()=>void cancelArchivePreview()}>
              {archiveCancelRequested?'Stopping archive preview…':'Cancel archive preview'}
            </button>}
            <button type="button" className="subtleButton" disabled={working || chatBusy} onClick={inspectSelectedTextFile} title="Choose a local text file in a native file picker and inspect it using Lu’s explicitly granted read-only native tool">Inspect file</button>
            <button type="button" className="subtleButton" disabled={!loaded}
              onClick={()=>void attachReaderDocument()}>+ Read file</button>
            <button type="button" className="subtleButton"
              disabled={!loaded||chatBusy||working||!prompt.trim()}
              title="Select one document and allow a read-only AI Reader tool turn"
              onClick={()=>void askReaderDocument()}>Ask Reader</button>
            <button type="button" className="subtleButton"
              disabled={!loaded||chatBusy||working||!prompt.trim()}
              title="Select one folder and let Lu read, search and inspect ZIP metadata inside it for this turn only"
              onClick={()=>void askWorkspaceDirectory()}>Ask Workspace</button>
            <button type="button" className="subtleButton"
              aria-pressed={workspaceToolsNextSend}
              disabled={!loaded||chatBusy||working||Boolean(images.length)||Boolean(textFiles.length)}
              title="On the next normal Send, choose a folder in the native picker and let Lu use approved read-only tools for that one turn"
              onClick={()=>{setBrowserResearchNextSend(false);setWorkspaceToolsNextSend(value=>!value);}}>
              {workspaceToolsNextSend?'Workspace tools: next Send ✓':'Use workspace tools on Send'}
            </button>
            {workspaceToolsNextSend && <span className="composerHelp" role="status">
              Next Send will open a native folder picker. Read-only access is granted for that turn only.
            </span>}
            <button type="button" className="subtleButton"
              aria-pressed={browserResearchNextSend}
              disabled={!loaded||chatBusy||working||Boolean(images.length)||Boolean(textFiles.length)}
              title="Next Send: request native approval for exact HTTPS origins, use a supervised read-only Browser and return a journaled answer"
              onClick={()=>{setWorkspaceToolsNextSend(false);setBrowserResearchNextSend(v=>!v);}}>
              {browserResearchNextSend?'Browser research: next Send ✓':'Research with Lu browser (read only)'}
            </button>
            {browserResearchNextSend&&<label className="composerHelp">
              Research HTTPS origins (exact public sites)
              <input value={browserOrigins} maxLength={2048}
                onChange={e=>setBrowserOrigins(e.target.value)}
                placeholder="https://example.org" />
              <small>Lu asks for native approval of the entire list.
                Text-only, read-only, one conversation turn.</small>
            </label>}
            <button type="button" className="subtleButton"
              disabled={!chatId}
              title="Inspect actual persisted Reader/Workspace model tool loops without replaying effects"
              onClick={()=>{setToolActivityOpen(v=>!v);void refreshToolActivity(chatId);}}>
              {toolActivityOpen?'Hide tool activity':'Tool activity'}
            </button>
            <span className="composerHelp">Explicit workspace searches and document-reading requests open native pickers; Git status, log and diff require selecting a repository · No automatic file permission</span>
            <span className="composerHelp">Paste or drop images and text · Enter to send · Shift+Enter for newline</span>
            {chatBusy ? <button className="dangerButton" type="button" onClick={cancel}
                disabled={!visibleJob.generationId || visibleJob.cancelling}>{visibleJob.cancelling ? 'Stopping…' : visibleJob.phase === 'queued' ? 'Cancel queued' : 'Cancel'}</button>
              : <button className="primaryButton" type="submit" disabled={working || !loaded || !prompt.trim()}>Send</button>}
          </div></form>}
      </div>
    </main>
    {viewedDraftImage?.previewUrl && <div className="imageViewerBackdrop" role="presentation"
      onClick={()=>setImageViewer(null)}>
      <section className="imageViewerDialog" role="dialog" aria-modal="true"
        aria-label="Image Workspace preview" onClick={e=>e.stopPropagation()}>
        <header>
          <strong>Image Workspace · original and immutable revision export</strong>
          <button type="button" aria-label="Close image preview"
            onClick={()=>setImageViewer(null)}>×</button>
        </header>
        <small>{viewedDraftImage.name||'Attached image'} · {viewedDraftImage.detectedMimeType||viewedDraftImage.mimeType||'Verified image'}</small>
        <div className="imageViewerViewport" aria-label="Zoomable original draft image">
          <img src={viewedDraftImage.previewUrl} alt="Original attached image, unmodified"
            onLoad={e=>{
              const width=e.currentTarget.naturalWidth,height=e.currentTarget.naturalHeight;
              setImageViewer(v=>v&&v.imageRef===viewedDraftImage.imageRef
                ?({...v,width,height}):v);
              setImageEdit({mode:'resize',x:'0',y:'0',width:String(width),height:String(height)});
              setImageExportReceipt(null);
            }}
            style={{transform:'scale('+imageViewer.zoom+')'}}/>
        </div>
        <footer>
          <button type="button" disabled={imageViewer.zoom<=0.5}
            onClick={()=>setImageViewer(v=>v&&({...v,zoom:Math.max(0.5,+(v.zoom-0.25).toFixed(2))}))}>Zoom out</button>
          <span>{Math.round(imageViewer.zoom*100)}%</span>
          <button type="button" disabled={imageViewer.zoom>=3}
            onClick={()=>setImageViewer(v=>v&&({...v,zoom:Math.min(3,+(v.zoom+0.25).toFixed(2))}))}>Zoom in</button>
          <button type="button" onClick={()=>setImageViewer(null)}>Close</button>
        </footer>
        <div className="imageRevisionForm" aria-label="Deterministic image revision export">
          <small>Original stays unchanged. Native approval and a NEW PNG destination are required.
            Solid masks remove pixels in the exported file; mosaics are visual effects, not secure redaction.
            AI-generated editing and automatic source overwrite remain disabled.</small>
          <label>Operation
            <select value={imageEdit.mode} disabled={imageExportBusy}
              onChange={e=>setImageEdit(v=>({...v,mode:e.target.value}))}>
              <option value="resize">Resize</option>
              <option value="crop">Crop</option>
              <option value="rotate90">Rotate 90°</option>
              <option value="flip_h">Flip horizontally</option>
              <option value="redact">Solid privacy mask</option>
              <option value="pixelate">Pixelate selected region</option>
              <option value="rectangle">Outline annotation</option>
            </select>
          </label>
          {['crop','redact','pixelate','rectangle'].includes(imageEdit.mode)&&<div className="imageRevisionDimensions">
            <label>X <input type="number" min="0" value={imageEdit.x}
              onChange={e=>setImageEdit(v=>({...v,x:e.target.value}))}/></label>
            <label>Y <input type="number" min="0" value={imageEdit.y}
              onChange={e=>setImageEdit(v=>({...v,y:e.target.value}))}/></label>
          </div>}
          {!['rotate90','flip_h'].includes(imageEdit.mode)&&<div className="imageRevisionDimensions">
            <label>Width <input type="number" min="1" max="8192" value={imageEdit.width}
              onChange={e=>setImageEdit(v=>({...v,width:e.target.value}))}/></label>
            <label>Height <input type="number" min="1" max="8192" value={imageEdit.height}
              onChange={e=>setImageEdit(v=>({...v,height:e.target.value}))}/></label>
          </div>}
          {imageEdit.mode==='pixelate'&&<label>Mosaic block size
            <input type="number" min="2" max="128" value={imageEdit.blockSize}
              onChange={e=>setImageEdit(v=>({...v,blockSize:e.target.value}))}/>
          </label>}
          {imageEdit.mode==='rectangle'&&<div className="imageRevisionDimensions">
            <label>Annotation color <input type="color" value={imageEdit.color}
              onChange={e=>setImageEdit(v=>({...v,color:e.target.value}))}/></label>
            <label>Stroke width <input type="number" min="1" max="64" value={imageEdit.stroke}
              onChange={e=>setImageEdit(v=>({...v,stroke:e.target.value}))}/></label>
          </div>}
          <button type="button" disabled={!chatId||!imageViewer?.width||imageExportBusy}
            onClick={()=>void exportImageRevision()}>
            {imageExportBusy?'Awaiting native approval…':'Save new PNG revision…'}
          </button>
          {imageExportReceipt&&<small role="status">
            New image version saved: {imageExportReceipt.fileName} ·
            {imageExportReceipt.width} × {imageExportReceipt.height} ·
            SHA-256 {imageExportReceipt.revisionSha256}. Original retained.
          </small>}
          <button type="button" disabled={!chatId||imageHistoryBusy}
            onClick={()=>void loadImageRevisionHistory()}>
            {imageHistoryBusy?'Reading history…':'Review saved revision history'}
          </button>
          {imageHistoryView.chatId===chatId&&
            <div aria-label="Historical image revision receipts">
              <small>Historical exports only. This list does not prove files still exist.
                Uncertain exports are never automatically retried.</small>
              <ul>{imageHistoryView.rows.map(record=><li key={record.id}>
                {record.fileName} · {record.width} × {record.height} ·
                {record.state} · SHA-256 {record.revisionSha256}
              </li>)}</ul>
            </div>}
        </div>
      </section>
    </div>}
    {dragActive && <div className="appDropHint" role="status" aria-live="polite">Drop image or text files to attach to this conversation</div>}
    {backgroundPaneOpen && <div className="backgroundPaneBackdrop" role="presentation"
      onClick={()=>setBackgroundPaneOpen(false)}>
      <aside className="backgroundPane" role="dialog" aria-modal="true"
        aria-label="Background Tasks" onClick={e=>e.stopPropagation()}>
        <header className="backgroundPaneHeader">
          <div><h2>Background Tasks</h2><small>Monitored by Lu — no model polling</small></div>
          <button className="quietIcon" aria-label="Close Background Tasks"
            onClick={()=>setBackgroundPaneOpen(false)}>×</button>
        </header>
        <div className="backgroundContinuationPreference">
          <label>
            <input type="checkbox" disabled={!chatId}
              checked={Boolean(chatId&&autoBackgroundContinuation)}
              onChange={e=>void changeAutoBackgroundContinuation(e.target.checked)}/>
            Automatically notify this conversation's AI when a task ends
          </label>
          <small>Off by default. When enabled, Lu may start a short AI reply after completion, failure, cancellation, interruption, or timeout. Does not grant tool permissions or load a model.</small>
        </div>
        <div className="backgroundScanLaunch">
          <button type="button" className="primaryButton"
            onClick={()=>void startMediaFileScan()}>
            Scan media file…
          </button>
          <small>Native worker · file integrity scan (SHA-256), not video content analysis</small>
        </div>
        {backgroundTerminalNotice && <div className="backgroundTerminalBanner" role="status">
          <strong>Task ended: {backgroundTerminalNotice.state}</strong>
          <span>{backgroundTerminalNotice.taskId}</span>
          <button type="button" className="quietIcon"
            aria-label="Dismiss terminal notification"
            onClick={()=>setBackgroundTerminalNotice(null)}>×</button>
        </div>}
        <div className="backgroundPaneToolbar">
          <div className="backgroundPaneFilters" aria-label="Task history filter">
            <button className={backgroundFilter==='active'?'selected':''}
              onClick={()=>setBackgroundFilter('active')}>Active ({activeBackgroundTasks.length})</button>
            <button className={backgroundFilter==='all'?'selected':''}
              onClick={()=>setBackgroundFilter('all')}>Recent</button>
          </div>
          <button className="subtleButton" onClick={()=>void refreshBackgroundTasks()}
            aria-label="Refresh background tasks">Refresh</button>
        </div>
        {backgroundError && <p className="backgroundPaneError" role="alert">{backgroundError}</p>}
        <div className="backgroundPaneList" aria-live="off">
          {shownBackgroundTasks.map(task=><article className="backgroundTask" key={task.taskId}>
            <div className="backgroundTaskHeading">
              <strong title={task.taskId}>{task.taskId}</strong>
              <span className={'backgroundTaskStatus state-'+task.state.toLowerCase()}>{task.state}</span>
            </div>
            {task.workspaceId && <small>Workspace: {task.workspaceId}</small>}
            <p>{task.detail || 'No progress details yet'}</p>
            <div className="backgroundTaskProgress" role="progressbar"
              aria-label={'Progress for '+task.taskId} aria-valuemin={0}
              aria-valuemax={100} aria-valuenow={task.progress}>
              <span style={{width:Math.max(0,Math.min(100,task.progress))+'%'}}/>
            </div>
            <div className="backgroundTaskFoot">
              <span>{task.progress}% recorded</span><span>Updated: {task.updatedAt || 'unknown'}</span>
            </div>
            {['INTERRUPTED','FAILED'].includes(task.state) &&
              <p className="backgroundTaskWarning">Requires review before retrying.</p>}
            {task.canCancel && <button className="backgroundTaskCancel" type="button"
              onClick={()=>void cancelBackgroundTask(task.taskId)}>
              Cancel task…
            </button>}

          </article>)}
          {!shownBackgroundTasks.length && !backgroundError &&
            <p className="backgroundPaneEmpty">
              {backgroundFilter==='active'?'No active background tasks.':'No background task history yet.'}
            </p>}
        </div>
        <footer className="backgroundPaneFooter">
          Cancel is available only for Lu-owned live workers and requires native confirmation.
          Pause and resume remain unavailable until workers support them.
          Ordinary progress updates do not invoke the AI model.
        </footer>
      </aside>
    </div>}
    {settingsOpen && <div className="settingsBackdrop" role="presentation" onClick={() => setSettingsOpen(false)}>
      <div className="settingsPane" role="dialog" aria-label="Lu settings" aria-modal="true" onClick={e => e.stopPropagation()}>
        <header><div><h2>Settings</h2><small>Local model management</small></div><button className="quietIcon" onClick={() => setSettingsOpen(false)} aria-label="Close settings">×</button></header>
        <button type="button" className="subtleButton full"
          aria-expanded={advancedSettingsOpen} onClick={() => setAdvancedSettingsOpen(v => !v)}>
          {advancedSettingsOpen ? 'Hide advanced settings' : 'Advanced settings'}
        </button>
        {advancedSettingsOpen && <section className="advancedDocumentSettings"
          aria-label="Default document folder settings">
          <h3>Documents and saved files</h3>
          <p>New text documents save to your user Documents folder by default.
            You may choose a different default folder with the native folder picker.</p>
          <small>System Documents: {documentSettings?.systemDocumentsFolder || 'Unavailable'}</small>
          <code className="writeTarget">{documentSettings?.defaultFolder || 'No default directory resolved'}</code>
          {!documentSettings?.isAvailable &&
            <small role="alert">The default folder is missing or unavailable. Select a valid folder before saving.</small>}
          <div className="settingsActions">
            <button type="button" className="subtleButton" onClick={chooseDefaultDocumentsFolder}>
              Choose default folder
            </button>
            <button type="button" className="subtleButton" onClick={resetDefaultDocumentsFolder}
              disabled={!documentSettings?.isCustom}>
              Reset to system Documents
            </button>
          </div>
          <p className="settingsNotice">Lu saves only after reviewing the exact file contents and destination.
            Modifying existing files requires a separate validated edit/backup pipeline.</p>
        </section>}
        <section className="advancedDocumentSettings" aria-label="Browser visibility settings">
          <h3>Browser visibility</h3>
          <p>Automatic uses a headed Chromium window hidden by default until the user is needed.
             Always Visible is optional. This preference does not launch or authorize browsing.</p>
          <label htmlFor="browserVisibilityMode">Window visibility</label>
          <select id="browserVisibilityMode" value={browserVisibility}
            onChange={e=>{
              const mode=e.target.value;
              invoke('lu_set_browser_visibility',{mode})
                .then(saved=>{setBrowserVisibility(saved);setBrowserPreferenceError('');})
                .catch(error=>setBrowserPreferenceError(String(error)));
            }}>
            <option value="automatic_hidden">Automatic (headed, initially hidden)</option>
            <option value="always_visible">Always visible</option>
          </select>
          {browserPreferenceError&&<small role="alert">{browserPreferenceError}</small>}
          <small>{browserRuntime?.reason||'Checking native browser runtime admission…'}</small>
          <div className="settingsActions">
            <button type="button" className="subtleButton"
              disabled={browserBusy} onClick={()=>browserOperation('lu_browser_runtime_status')}>
              Check Browser runtime
            </button>
          </div>
          <label htmlFor="browserTrustedOrigins">HTTPS origins for a new read-only task</label>
          <input id="browserTrustedOrigins" value={browserOrigins}
            onChange={e=>setBrowserOrigins(e.target.value)} disabled={browserBusy||browserRuntime?.running}
            placeholder="https://example.org" />
          <div className="settingsActions">
            {!browserRuntime?.running &&
              <button type="button" disabled={browserBusy||!chatId||!browserRuntime?.launchReady}
                onClick={()=>browserOperation('lu_browser_start',{chatId,origins:browserOrigins})}>
                Authorize read-only Browser
              </button>}
            {browserRuntime?.running && <>
              <button type="button" disabled={browserBusy||browserReadBusy||!runtime.sessionId
                  ||browserRuntime.humanVerificationRequired}
                onClick={()=>void inspectWithBrowserModel()}>
                {browserReadBusy?'Running supervised Browser model read…':
                  'Let Lu inspect this approved browser'}
              </button>

              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_set_visible',{visible:true})}>
                Show browser
              </button>
              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_set_visible',{visible:false})}>
                Hide browser
              </button>
              {browserRuntime.humanVerificationRequired &&
                <button type="button" disabled={browserBusy}
                  onClick={()=>browserOperation('lu_browser_human_verified')}>
                  I've completed verification
                </button>}
              <button type="button" disabled={browserBusy}
                onClick={()=>browserOperation('lu_browser_stop')}>Stop browser</button>
            </>}
          </div>
          <small>Browser navigation requires a currently authenticated human conversation and separate native approval.
            Lu's supervised read-only inspection uses the same approved turn and exact origins. It grants no form, click, typing or additional network authority.</small>
          {browserReadError&&<small role="alert">{browserReadError}</small>}
          {browserReadResult?.chatId===chatId&&<section aria-label="Model-guided browser observations">
            <strong>{browserReadResult.proposalsExecuted} supervised Browser operations</strong>
            <small>{browserReadResult.message}</small>
            <ul>{browserReadResult.observations.map((page,index)=><li key={index}>
              <strong>{page.status}</strong>
              {page.url&&<small>{page.url}</small>}
              {page.excerpt&&<pre>{page.excerpt}</pre>}
              {page.truncated&&<small>Page excerpt truncated.</small>}
            </li>)}</ul>
          </section>}
        </section>
        <section className="advancedDocumentSettings" aria-label="Research discovery">
          <h3>Research · verified-evidence workflow</h3>
          <p>OpenAlex discovery is a separately approved read-only operation. Search results are
             unverified leads, never citations, until registry and document evidence are checked.</p>
          <div className="settingsActions">
            <button type="button" disabled={!chatId||browserEvidenceBusy}
              onClick={()=>void reviewBrowserEvidence()}>
              {browserEvidenceBusy?'Reading evidence…':'Review observed Browser evidence'}
            </button>
          </div>
          <small>Browser reads are local, untrusted observations, not independently
            corroborated sources or verified academic citations. Their exact
            post-redirect URLs and text digests come from Lu's supervised worker.</small>
          {Array.isArray(browserEvidenceByChat[chatId])&&
            <div className="researchResults" aria-label="Observed Browser evidence">
              <strong>{browserEvidenceByChat[chatId].length} recent Browser observations · UNCORROBORATED</strong>
              <ul>{browserEvidenceByChat[chatId].map(item=><li key={item.callId}>
                <strong>{item.sourceUrl}</strong>
                <small>Observed SHA-256: {item.observedSha256}</small>
                <small>{item.verificationStatus} · citation verified: no</small>
                <pre>{item.excerpt.slice(0,1500)}</pre>
                {item.excerptTruncated&&<small>Excerpt incomplete; only saved text can be checked.</small>}
                <label>Exact passage from this observed excerpt
                  <input value={researchQuoteDraftByKey[chatId+'|'+item.generationId+'|'+item.callId]||''}
                    onChange={e=>{
                      const key=chatId+'|'+item.generationId+'|'+item.callId;
                      setResearchQuoteDraftByKey(old=>({...old,[key]:e.target.value}));
                      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
                    }} placeholder="Paste a verbatim quotation" />
                </label>
                <label>Optional independently corroborated OpenAlex source ID
                  <input value={researchQuoteSourceByKey[chatId+'|'+item.generationId+'|'+item.callId]||''}
                    onChange={e=>{
                      const key=chatId+'|'+item.generationId+'|'+item.callId;
                      setResearchQuoteSourceByKey(old=>({...old,[key]:e.target.value}));
                      setResearchQuoteProofByKey(old=>({...old,[key]:null}));
                    }} placeholder="Only if Crossref metadata was checked" />
                </label>
                <button type="button" disabled={researchQuoteBusy||
                  (researchQuoteDraftByKey[chatId+'|'+item.generationId+'|'+item.callId]||'').trim().length<4}
                  onClick={()=>void verifyObservedQuote(item)}>
                  Check exact source passage
                </button>
                {researchQuoteProofByKey[chatId+'|'+item.generationId+'|'+item.callId]&&
                  <small>{researchQuoteProofByKey[chatId+'|'+item.generationId+'|'+item.callId].message}</small>}
              </li>)}</ul>
            </div>}
          <label htmlFor="researchQuery">Research question or subject</label>
          <input id="researchQuery" value={researchQuery} disabled={researchBusy}
            onChange={e=>{setResearchQuery(e.target.value);setResearchResult(null);}}
            placeholder="e.g., spatial reasoning and GUI automation" />
          <div className="settingsActions">
            <button type="button" disabled={!chatId||!researchAvailable||researchBusy||!researchQuery.trim()}
              onClick={runResearchDiscovery}>
              {researchBusy?'Searching reviewed source…':'Search OpenAlex'}
            </button>
          </div>
          <small>{researchAvailable?'Pinned HTTPS research runtime available. Native consent required.'
            :'Research network runtime is not bundled in this build; discovery remains disabled.'}</small>
          <label>Maximum distinct DOI leads (up to 120)
            <input type="number" min="1" max="120" value={researchCampaignCeiling}
              onChange={e=>setResearchCampaignCeiling(Math.min(120,Math.max(1,
                Number(e.target.value)||1)))} />
          </label>
          <div className="settingsActions">
            <button type="button" disabled={!chatId||!researchAvailable||
              !researchQuery.trim()||researchCampaignBusy}
              onClick={()=>void runResearchCampaign()}>
              {researchCampaignBusy?'Researching in approved catalog…':'Run bounded Research campaign'}
            </button>
            <button type="button" disabled={!chatId||researchCampaignBusy}
              onClick={()=>void refreshResearchCampaigns()}>
              Restore saved research jobs
            </button>
          </div>
          {Array.isArray(researchCampaignsByChat[chatId])&&
            <div className="researchResults" aria-label="Durable research campaigns">
              <ul>{researchCampaignsByChat[chatId].map(job=><li key={job.campaignId}>
                <strong>{job.query}</strong>
                <small>{job.state} · {job.distinctDois}/{job.candidateCeiling}
                  distinct DOI leads · {job.completedPages} pages checkpointed</small>
                <small>{job.message}</small>
                {job.blockedReason&&<small>Blocker: {job.blockedReason}</small>}
                {['PAUSED','BLOCKED','RUNNING'].includes(job.state)&&
                  <button type="button" disabled={researchCampaignBusy||!researchAvailable}
                    onClick={()=>void resumeResearchCampaign(job.campaignId)}>
                    Reauthorize and resume
                  </button>}
                {!['DONE','CANCELLED'].includes(job.state)&&
                  <button type="button" disabled={researchCampaignBusy}
                    onClick={()=>void cancelResearchCampaign(job.campaignId)}>
                    Cancel campaign
                  </button>}
              </li>)}</ul>
            </div>}
          <div className="researchResults" aria-label="Source-linked claim matrix">
            <strong>Document-backed claim and counterevidence review</strong>
            <small>Only a previously verified full-text PDF passage can enter this matrix.
              Your interpretation is recorded as human-reviewed, not as established fact.</small>
            <input aria-label="Verified research source ID"
              placeholder="Crossref-corroborated OpenAlex source ID"
              value={researchClaimDraft.sourceId}
              onChange={e=>setResearchClaimDraft(old=>({...old,sourceId:e.target.value}))}/>
            <input aria-label="Claim under analysis" placeholder="Specific claim"
              value={researchClaimDraft.claim}
              onChange={e=>setResearchClaimDraft(old=>({...old,claim:e.target.value}))}/>
            <textarea aria-label="Verified quote for research claim"
              placeholder="Exact verified PDF passage" value={researchClaimDraft.quote}
              onChange={e=>setResearchClaimDraft(old=>({...old,quote:e.target.value}))}/>
            <label>Evidence relationship
              <select value={researchClaimDraft.stance}
                onChange={e=>setResearchClaimDraft(old=>({...old,stance:e.target.value}))}>
                <option value="SUPPORTS">Supports</option>
                <option value="CHALLENGES">Challenges</option>
                <option value="COMPLICATES">Qualifies or complicates</option>
              </select>
            </label>
            <input aria-label="Reasoning for interpretation"
              placeholder="Why this quote relates to the claim"
              value={researchClaimDraft.reasoning}
              onChange={e=>setResearchClaimDraft(old=>({...old,reasoning:e.target.value}))}/>
            <div className="settingsActions">
              <button type="button" disabled={!chatId||researchReportBusy||
                !researchClaimDraft.sourceId.trim()||!researchClaimDraft.claim.trim()||
                !researchClaimDraft.quote.trim()||!researchClaimDraft.reasoning.trim()}
                onClick={()=>void submitResearchClaim()}>
                Save my evidence interpretation
              </button>
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void refreshResearchClaims()}>
                Review saved claims
              </button>
            </div>
            {Array.isArray(researchClaimsByChat[chatId])&&
              <ul>{researchClaimsByChat[chatId].map(item=><li key={item.claim}>
                <strong>{item.claim}</strong>
                <small>Support: {item.supports} · Challenge: {item.challenges}
                  · Qualification: {item.complicates}
                  · Source IDs: {item.distinctCatalogSources}</small>
                {item.unresolvedDisagreement&&
                  <small>Unresolved: contrary or qualifying evidence remains</small>}
                <small>{item.reasonCodes.join(', ')}</small>
              </li>)}</ul>}
            <label>Verified evidence report title
              <input value={researchReportTitle}
                onChange={e=>setResearchReportTitle(e.target.value)}/>
            </label>
            <div className="settingsActions">
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void exportResearchReport('markdown')}>
                Save Markdown report
              </button>
              <button type="button" disabled={!chatId||researchReportBusy}
                onClick={()=>void exportResearchReport('json')}>
                Save evidence JSON
              </button>
            </div>
            {researchReportSavedByChat[chatId]&&
              <small>Saved {researchReportSavedByChat[chatId].format}:
                {' '}{researchReportSavedByChat[chatId].savedPath}
                {' '}· SHA-256: {researchReportSavedByChat[chatId].savedSha256}
                {' '}· {researchReportSavedByChat[chatId].checkedPassages}
                verified passages. Interpretations are not independently certified.</small>}
          </div>
          {researchError&&<small role="alert">{researchError}</small>}
          {researchResult?.chatId===chatId&&<div className="researchResults" aria-label="Research evidence leads">
            <strong>{researchResult.sourceCount} discovered sources · UNVERIFIED</strong>
            <small>{researchResult.message}</small>
            <ul>{researchResult.leads.slice(0,25).map(l=><li key={l.sourceId}>
              <strong>{l.title}</strong>
              <small>Source ID: {l.sourceId}</small>
              <small>{l.authors} · DOI {l.doi} · {
                researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus || l.verificationStatus
              }</small>
              <button type="button" disabled={researchBusy||!researchAvailable}
                onClick={()=>void checkResearchMetadata(l.sourceId)}>
                Check independent Crossref metadata
              </button>
              {researchMetadataByChat[chatId+'|'+l.sourceId]&&
                <small>{researchMetadataByChat[chatId+'|'+l.sourceId].message}</small>}
              {researchMetadataByChat[chatId+'|'+l.sourceId]?.verificationStatus==='metadata_corroborated'&&
                <button type="button" disabled={researchBusy||!researchAvailable}
                  onClick={()=>void retrieveResearchDocument(l.sourceId)}>
                  Retrieve bounded PDF/HTML document
                </button>}
              {researchDocumentsByChat[chatId+'|'+l.sourceId]&&
                <div className="researchResults" aria-label="Retrieved research document">
                  <small>{researchDocumentsByChat[chatId+'|'+l.sourceId].message}</small>
                  <small>Final HTTPS URL: {researchDocumentsByChat[chatId+'|'+l.sourceId].finalUrl}</small>
                  <small>Redirect-chain SHA-256: {researchDocumentsByChat[chatId+'|'+l.sourceId].redirectChainSha256}</small>
                  <small>{researchDocumentsByChat[chatId+'|'+l.sourceId].format} · {researchDocumentsByChat[chatId+'|'+l.sourceId].documentAccessed?'Document text accessed':'Unverified document candidate'}</small>
                  <pre>{researchDocumentsByChat[chatId+'|'+l.sourceId].excerpt}</pre>
                  {researchDocumentsByChat[chatId+'|'+l.sourceId].documentAccessed&&
                    <>
                      <input aria-label="Exact quote from retrieved document"
                        value={researchDocumentQuotes[chatId+'|'+l.sourceId]||''}
                        onChange={e=>{
                          const key=chatId+'|'+l.sourceId;
                          setResearchDocumentQuotes(old=>({...old,[key]:e.target.value}));
                          setResearchDocumentPassages(old=>({...old,[key]:null}));
                        }} placeholder="Copy an exact PDF passage" />
                      <button type="button" disabled={researchBusy||
                        !(researchDocumentQuotes[chatId+'|'+l.sourceId]||'').trim()}
                        onClick={()=>void verifyResearchDocumentPassage(l.sourceId)}>
                        Verify stored PDF passage
                      </button>
                      {researchDocumentPassages[chatId+'|'+l.sourceId]&&
                        <small>Passage verified against stored PDF text; model interpretation remains unverified.</small>}
                    </>}
                </div>}
            </li>)}</ul>
            <div className="settingsActions">
              <button type="button" disabled={researchBusy||researchResult.page<=1}
                onClick={()=>void runResearchDiscovery(researchResult.page-1)}>
                Previous discovery page
              </button>
              <small>Page {researchResult.page} of at most 5 · leads are not citations</small>
              <button type="button" disabled={researchBusy||!researchResult.nextPage}
                onClick={()=>void runResearchDiscovery(researchResult.nextPage)}>
                Next discovery page
              </button>
            </div>
          </div>}
        </section>
        <h3>Models</h3><p>GGUF files in Lu's models folder are discovered at startup, but <strong>never loaded automatically</strong>.</p>
        <button className="subtleButton full" onClick={rescanModels} disabled={busy || loaded}>Rescan models folder</button>
        <button className="subtleButton full" onClick={importModel} disabled={busy}>Import GGUF from another location</button>
        <div className="settingsModelList">{models.map(m => <button key={m.modelRef} className={'settingsModel '+(selectedModel === m.modelRef ? 'selected' : '')}
          onClick={() => setSelectedModel(m.modelRef)} disabled={busy}>
          <strong>{m.filename}</strong><small>{m.projectorAttached ? 'GGUF + MMProj' : 'GGUF only'} · {m.available ? 'Available' : 'Missing file'}</small></button>)}</div>
        <button className="subtleButton full" disabled={busy || loaded || !selected || selected.projectorAttached} onClick={attachProjector}>Attach MMProj to selected model</button>
        <div className="settingsActions"><button className="primaryButton" disabled={busy || loaded || !selected?.available} onClick={loadModel}>Load selected model</button>
          <button className="subtleButton" disabled={busy || !loaded} onClick={unloadModel}>Unload current model</button></div>
        <div className="settingsStatus"><span className="statusDot" data-ready={loaded}></span>{activity || (loaded ? 'Ready' : 'No model loaded')}</div>
        <p className="settingsNotice">Vulkan acceleration is requested; device offload requires verification from the engine logs. Model selection never activates weights.</p>
      </div>
    </div>}
  </div>;
}

createRoot(document.getElementById('root')).render(<App />);
