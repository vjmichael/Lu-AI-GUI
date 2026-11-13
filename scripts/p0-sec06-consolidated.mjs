#!/usr/bin/env node
// SEC-06 consolidated, fail-closed native security acceptance coordinator.
// This is an explicitly invoked developer/owner tool. It never dispatches
// CI, merges a PR, modifies a production app, or grants execution authority.
// A successful source test is NOT proof of Windows Job or Linux cgroup custody.
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {constants,existsSync,accessSync,mkdirSync,readFileSync,writeFileSync,mkdtempSync} from 'node:fs';
import {join,dirname,resolve} from 'node:path';
import {tmpdir,platform,arch,release} from 'node:os';
import {fileURLToPath} from 'node:url';
import {canonicalSourceLockSha256} from './sec06-source-lock-digest.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const argv=process.argv.slice(2);
const arg=(flag)=>{const i=argv.indexOf(flag);return i>=0?argv[i+1]:null};
const mode=arg('--mode');
const supported=['source','linux','windows','verify'];
if (!supported.includes(mode)){
  process.stderr.write('Usage: node scripts/p0-sec06-consolidated.mjs --mode source|linux|windows|verify [--evidence DIR] [--source RECEIPT --linux RECEIPT --windows RECEIPT]\n');
  process.exit(2);
}
const hash=(bytes)=>createHash('sha256').update(bytes).digest('hex');
const stringHash=(s)=>hash(Buffer.from(s,'utf8'));
const command=(exe,args,timeout=180000,environment=process.env)=>{
  const r=spawnSync(exe,args,{
    cwd:root,env:environment,encoding:'utf8',
    timeout,maxBuffer:8*1024*1024,windowsHide:true,
  });
  return {code:r.status,signal:r.signal,
    error:r.error?String(r.error):null,
    output:'STDOUT\n'+(r.stdout||'')+'\nSTDERR\n'+(r.stderr||'')+'\n'};
};
const git=(...a)=>{
  const r=command('git',a,30000);
  if(r.code!==0||r.error)throw new Error('SEC06_SOURCE_IDENTITY_UNAVAILABLE: '+r.output.slice(0,1000));
  return r.output.split('\nSTDERR\n')[0].replace(/^STDOUT\n/,'').trim();
};
const sha=/^[a-f0-9]{64}$/;
const receiptId=/^[a-f0-9]{40}$/;
const lockSha=canonicalSourceLockSha256(root);
let currentHead;
try{currentHead=git('rev-parse','HEAD')}catch(e){
  process.stderr.write(String(e)+'\n');process.exit(2);
}
if(!receiptId.test(currentHead)){throw new Error('Invalid original source commit identity')}
const approvedEvidenceRoot=arg('--evidence');
const evidence=approvedEvidenceRoot?resolve(approvedEvidenceRoot):mkdtempSync(join(tmpdir(),'lu-sec06-'));
if(mode!=='verify')mkdirSync(evidence,{recursive:true});
const results=[];
let blocked=false;
function doCase(id,exe,args,opts={}){
  const env=opts.env||process.env;
  const result=command(exe,args,opts.timeout||180000,env);
  const log=join(evidence,id+'.log');
  const body='COMMAND '+[exe,...args].join(' ')+'\n'+result.output;
  writeFileSync(log,body,{flag:'wx',mode:0o600});
  const evidenceDigest=stringHash(body);
  let status=result.code===0&&!result.error?'PASS':
    result.code===77||result.error&&result.error.includes('ENOENT')?'NOT_AVAILABLE':'FAIL';
  if(status==='PASS'&&opts.mustContain&&
     !opts.mustContain.every(t=>result.output.includes(t)))status='FAIL';
  if(status==='PASS'&&opts.mustReject&&
     opts.mustReject.some(t=>result.output.includes(t)))status='FAIL';
  if(status==='PASS'&&opts.nodeTestSummary){
    const count=/# tests\s+(\d+)/.exec(result.output);
    const success=/# pass\s+(\d+)/.exec(result.output);
    const failures=/# fail\s+(\d+)/.exec(result.output);
    if(!count||!success||!failures||Number(count[1])<=0||
       Number(success[1])!==Number(count[1])||Number(failures[1])!==0){
      status='FAIL';
    }
  }
  if(status==='PASS'&&opts.rustTestSummary){
    const matches=[...result.output.matchAll(/test result: ok\. (\d+) passed; (\d+) failed/g)];
    if(!matches.some(m=>Number(m[1])>0&&Number(m[2])===0)){
      status='FAIL';
    }
  }
  if(status!=='PASS')blocked=true;
  const row={id,status,exitCode:result.code,signal:result.signal,
    stderrOrSpawnError:result.error,log:id+'.log',sha256:evidenceDigest};
  results.push(row);
  process.stdout.write(id+' '+status+'\n');
  return row;
}
function writeReceipt(target){
  let delegationWitnessSha256=null;
  if(target==='linux'){
    try{
      const witnessPath=join(evidence,'kernel-delegation.json');
      const bytes=readFileSync(witnessPath);
      const proof=JSON.parse(bytes.toString('utf8'));
      if(proof.schema!=='lu-sec06c-native-delegation-v1'||
         proof.status!=='DELEGATED_KERNEL_PREFLIGHT_ONLY'||
         proof.head!==currentHead||!Number.isInteger(proof.euid)||
         proof.euid<=0||proof.realDelegation!==true||
         proof.systemdDelegateXattr!==true||
         proof.kernelCgroupKillPresent!==true||
         proof.probePopulatedZero!==true||
         proof.kernelTestAccepted!==false||
         proof.releaseOrInstallAuthorized!==false){
        throw new Error('invalid or not actually delegated OS preflight');
      }
      delegationWitnessSha256=hash(bytes);
    }catch(e){reject('SEC06C Linux delegated-kernel witness missing: '+String(e))}
  }
  const status=blocked?'BLOCKED_NATIVE_SECURITY':
    target==='source'?'SOURCE_CONTRACTS_PASS_NATIVE_UNPROVEN':
    'PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING';
  const receipt={
    schema:'lu-sec06-native-evidence-v1',
    target,head:currentHead,sourceLockSha256:lockSha,
    machine:{platform:platform(),arch:arch(),release:release()},
    status,allNativeCasesPassed:!blocked,
    ...(target==='linux'?{delegationWitnessSha256}:{}),
    localExecutionOnly:true,noReleaseAuthority:true,
    independentlyVerifiedInstalledHumanAcceptance:false,
    cases:results,
  };
  const path=join(evidence,target+'.receipt.json');
  writeFileSync(path,JSON.stringify(receipt,null,2)+'\n',{flag:'wx',mode:0o600});
  process.stdout.write('SEC06_RECEIPT='+path+'\nSEC06_STATUS='+status+'\n');
  if(blocked)process.exitCode=1;
}
function reject(reason){
  blocked=true;
  results.push({id:'preflight',status:'NOT_AVAILABLE',reason});
  process.stderr.write('SEC06_NOT_AVAILABLE '+reason+'\n');
}
function sourceCases(){
  doCase('exact-head-inventory','node',['scripts/verify-source-tree.mjs'],{timeout:120000});
  doCase('security-contracts','node',[
    '--test','lu-desktop/tests/p0-sec-guardian-hardening.test.mjs',
    'lu-desktop/tests/p0-sec05-adversarial.test.mjs',
    'lu-desktop/tests/p0-sec06-consolidated.test.mjs',
    'lu-desktop/tests/reviewed-app-desktop.test.mjs',
    'lu-desktop/tests/reviewed-task-continuity.test.mjs',
    'lu-desktop/tests/agent01l-os-custody.test.mjs',
    'lu-desktop/tests/agent01l-linux-cgroup-v2.test.mjs'
  ],{timeout:180000,nodeTestSummary:true});
  doCase('sec06-js-syntax','node',[
    '--check','scripts/p0-sec06-consolidated.mjs'
  ],{timeout:30000});
  if(platform()!=='win32'){
    doCase('linux-negative-python-syntax','python3',[
      '-m','py_compile','benchmarks/e1/p0_sec02_linux_guardian_negative.py'
    ],{timeout:30000});
    doCase('linux-host-crash-python-syntax','python3',[
      '-m','py_compile','benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py'
    ],{timeout:30000});
    doCase('linux-delegated-python-syntax','python3',[
      '-m','py_compile','scripts/sec06-linux-delegated-runner.py'
    ],{timeout:30000});
    doCase('linux-root-owned-prototype-syntax','python3',[
      '-m','py_compile','benchmarks/e1/sec06c_root_owned_systemd_boundary.py'
    ],{timeout:30000});
    doCase('linux-os-owned-git-syntax','python3',[
      '-m','py_compile','scripts/lu-sec06c-owned-git-launcher.py',
      'scripts/lu-sec06c-owned-git-worker.py'
    ],{timeout:30000});
    doCase('linux-os-owned-probe-syntax','python3',[
      '-m','py_compile','scripts/lu-sec06c-owned-probe-launcher.py',
      'scripts/lu-sec06c-owned-probe-worker.py'
    ],{timeout:30000});
    doCase('linux-os-owned-kernel-manager-syntax','python3',[
      '-m','py_compile','scripts/lu-sec06c-owned-kernel-manager.py'
    ],{timeout:30000});
  }
}
function rustCases(){
  for(const [id,manifest,filter] of [
    ['core-native-full','src-tauri/lu-core/Cargo.toml',null],
    ['desktop-ansf-native','lu-desktop/src-tauri/Cargo.toml',null],
    ['worker-job-native','src-tauri/lu-win-worker-job/Cargo.toml',null],
    ['reviewed-restart-negative','lu-desktop/src-tauri/Cargo.toml','sec04_'],
    ['adversarial-sql-and-revocation','lu-desktop/src-tauri/Cargo.toml','sec05_'],
  ]){
    const args=['test','--locked','--manifest-path',manifest];
    if(filter)args.push(filter);
    doCase(id,'cargo',args,{timeout:900000,rustTestSummary:true});
  }
  for(const [id,manifest] of [
    ['core-sec-regressions','src-tauri/lu-core/Cargo.toml'],
    ['desktop-sec-regressions','lu-desktop/src-tauri/Cargo.toml']
  ]){
    doCase(id,'cargo',[
      'test','--locked','--manifest-path',manifest,'sec0','--','--nocapture'
    ],{timeout:900000,rustTestSummary:true});
  }
  if(platform()==='linux'){
    doCase('worker-sec-regressions','cargo',[
      'test','--locked','--manifest-path',
      'src-tauri/lu-win-worker-job/Cargo.toml',
      '--lib','sec0','--','--nocapture'
    ],{timeout:600000,rustTestSummary:true});
  }
  const built=doCase('native-file-scan-build','cargo',[
    'build','--locked','--manifest-path',
    'lu-desktop/tests/native-file-scan/Cargo.toml'
  ],{timeout:600000});
  if(built.status==='PASS'){
    doCase('native-file-scan-e2e','node',[
      '--test','lu-desktop/tests/background-worker-e2e.test.mjs'
    ],{
      timeout:180000,nodeTestSummary:true,
      mustContain:['production scanner harness reads media-sized file',
                   'worker mode requires a single readable nonempty file']
    });
  }else{
    reject('native production scanner acceptance binary not built');
  }
}
if(mode==='verify'){
  const proofMap={source:arg('--source'),linux:arg('--linux'),windows:arg('--windows')};
  const summary={schema:'lu-sec06-consolidated-review-v1',head:currentHead,
    sourceLockSha256:lockSha,proofs:{},securityDecision:'BLOCKED',
    ownerOperatedInstalledAppAccepted:false,releaseOrMergeAuthorized:false};
  let valid=true;
  for(const [target,filename] of Object.entries(proofMap)){
    const info={status:'NOT_AVAILABLE'};
    if(filename){
      try{
        const path=resolve(filename);
        const p=JSON.parse(readFileSync(path,'utf8'));
        if(p.schema!=='lu-sec06-native-evidence-v1'||p.target!==target||
           p.head!==currentHead||p.sourceLockSha256!==lockSha||
           p.allNativeCasesPassed!==true||
           p.localExecutionOnly!==true||p.noReleaseAuthority!==true||
           p.independentlyVerifiedInstalledHumanAcceptance!==false||
           p.status!==(target==='source'?
             'SOURCE_CONTRACTS_PASS_NATIVE_UNPROVEN':
             'PROVISIONAL_NATIVE_PASS_OWNER_GATE_PENDING')){
          throw new Error('wrong original source identity or unaccepted native result');
        }
        if(target==='linux'){
          const dpath=join(dirname(path),'kernel-delegation.json');
          const dbytes=readFileSync(dpath);
          const witness=JSON.parse(dbytes.toString('utf8'));
          if(witness.schema!=='lu-sec06c-native-delegation-v1'||
             witness.status!=='DELEGATED_KERNEL_PREFLIGHT_ONLY'||
             witness.head!==currentHead||
             witness.euid<=0||!Number.isInteger(witness.euid)||
             witness.realDelegation!==true||witness.systemdDelegateXattr!==true||
             witness.kernelCgroupKillPresent!==true||witness.probePopulatedZero!==true||
             witness.kernelTestAccepted!==false||witness.releaseOrInstallAuthorized!==false||
             hash(dbytes)!==p.delegationWitnessSha256){
            throw new Error('missing or altered genuine Linux OS delegation witness');
          }
        }
        if(target==='linux'&&p.machine.platform!=='linux'||
           target==='windows'&&p.machine.platform!=='win32'){
          throw new Error('wrong original OS proof');
        }
        const required={
          source:['exact-head-inventory','security-contracts',
                  'sec06-js-syntax','linux-delegated-python-syntax',
                  'linux-root-owned-prototype-syntax',
                  'linux-os-owned-git-syntax','linux-os-owned-probe-syntax',
                  'linux-os-owned-kernel-manager-syntax'],
          linux:['exact-head-inventory','security-contracts',
                 'sec06-js-syntax','linux-negative-python-syntax',
                 'linux-host-crash-python-syntax','linux-delegated-python-syntax',
                 'linux-root-owned-prototype-syntax','linux-root-owned-unit-prototype',
                 'linux-root-owned-unit-cancel','linux-root-owned-unit-host-crash',
                 'linux-os-owned-git-syntax','linux-os-owned-probe-syntax',
                 'linux-os-owned-kernel-manager-syntax',
                 'core-native-full','desktop-ansf-native','worker-job-native',
                 'core-sec-regressions','desktop-sec-regressions',
                 'worker-sec-regressions',
                 'native-file-scan-build','native-file-scan-e2e',
                 'guardian-test-binaries','linux-original-host-sigkill',
                 'linux-reviewed-git-original-journal',
                 'linux-guardian-death','linux-guardian-stall',
                 'linux-worker-stall','linux-ancestor-migration'],
          windows:['exact-head-inventory','security-contracts',
                   'sec06-js-syntax','core-native-full',
                   'desktop-ansf-native','worker-job-native',
                   'core-sec-regressions','desktop-sec-regressions',
                   'native-file-scan-build','native-file-scan-e2e',
                   'windows-real-two-jobs',
                   'windows-reviewed-git-original-journal'],
        }[target];
        if(!Array.isArray(p.cases)||
           required.some(id=>!p.cases.some(x=>x.id===id&&x.status==='PASS'))||
           p.cases.some(x=>x.status!=='PASS'||!sha.test(x.sha256))){
          throw new Error('missing genuine required SEC06 native cases');
        }
        for(const entry of p.cases){
          if(typeof entry.log!=='string'||!/^[-a-z0-9]+\.log$/.test(entry.log)){
            throw new Error('unsafe or missing original case log');
          }
          const pathToLog=join(dirname(path),entry.log);
          if(!existsSync(pathToLog)||hash(readFileSync(pathToLog))!==entry.sha256){
            throw new Error('tampered original native log: '+entry.id);
          }
        }
        info.status='NATIVE_EVIDENCE_PRESENT_UNREVIEWED';
        info.cases=p.cases.length;
      }catch(e){info.status='UNTRUSTED_OR_INCOMPLETE';info.reason=String(e)}
    }
    if(info.status!=='NATIVE_EVIDENCE_PRESENT_UNREVIEWED')valid=false;
    summary.proofs[target]=info;
  }
  summary.securityDecision=valid?
    'NATIVE_CASES_PRESENT_OWNER_INSTALLED_AND_INDEPENDENT_REVIEW_PENDING':
    'BLOCKED_MISSING_OR_UNTRUSTED_NATIVE_PROOFS';
  process.stdout.write(JSON.stringify(summary,null,2)+'\n');
  // Even fully matching native logs cannot be auto-promoted to SEC-06 PASS:
  // only the owner/independent installed-app and kernel review can close it.
  process.exitCode=1;
}else{
  if(git('status','--porcelain','--untracked-files=no')){
    reject('working tree differs from original exact source head');
    writeReceipt(mode);
  }else if(mode==='source'){
    sourceCases();
    writeReceipt(mode);
  }else if(mode==='linux'&&platform()!=='linux'||
           mode==='windows'&&platform()!=='win32'){
    reject('requested operating system unavailable');
    writeReceipt(mode);
  }else{
    sourceCases();
    rustCases();
    if(mode==='linux'){
      const cgroup='/sys/fs/cgroup';
      if(!existsSync(join(cgroup,'cgroup.controllers'))){
        reject('unified Linux cgroup-v2 unavailable');
      }else if(typeof process.getuid!=='function'||process.getuid()===0){
        reject('destructive fixture must run as ordinary delegated user');
      }else{
        const build=doCase('guardian-test-binaries','cargo',[
          'build','--locked','--manifest-path',
          'src-tauri/lu-win-worker-job/Cargo.toml',
          '--features','e1-p08-crash-fixture',
          '--bin','lu-linux-cgroup-guard',
          '--bin','lu-p0-sec02-linux-negative',
          '--bin','lu-e1-p08-linux-cgroup-crash-parent'
        ],{timeout:900000});
        if(build.status==='PASS'){
          const basedir=join(root,'src-tauri/lu-win-worker-job/target/debug');
          const helper=join(basedir,'lu-linux-cgroup-guard');
          const fixture=join(basedir,'lu-p0-sec02-linux-negative');
          const crashHost=join(basedir,'lu-e1-p08-linux-cgroup-crash-parent');
          if(!existsSync(helper)||!existsSync(fixture)||
             !existsSync(crashHost)){
            reject('original compiled first-party helper binaries not found');
          }else{
            const env={...process.env,LU_AGENT01L_TEST_CGROUP_GUARD:helper};
            // Additional independent OS-owned candidate boundary: the
            // system manager retains the cgroup root, so an equal-UID
            // external attacker must not be able to write ancestor.procs.
            // This is an engineering prototype, NOT original Lu custody
            // acceptance; the original destructive fixtures remain below.
            doCase('linux-root-owned-unit-prototype','python3',[
              'benchmarks/e1/sec06c_root_owned_systemd_boundary.py',
              '--outer','--expected-sha',currentHead,
              '--evidence',join(evidence,'kernel-root-owned')
            ],{
              timeout:180000,env,
              mustContain:['SEC06C_ROOT_OWNED_UNIT_KERNEL_PROTOTYPE_PASS=1'],
              mustReject:['SEC06C_ROOT_OWNED_BOUNDARY_BLOCKED']
            });
            // Additional non-delegated, systemd-owned boundary interrupt
            // probes. These are separate cases, NOT substitutions for
            // the original Linux SEC-06 attacker/guardian matrix below.
            for(const [id,scenario] of [
              ['linux-root-owned-unit-cancel','cancel'],
              ['linux-root-owned-unit-host-crash','host-crash']
            ]){
              doCase(id,'python3',[
                'benchmarks/e1/sec06c_root_owned_systemd_boundary.py',
                '--outer','--expected-sha',currentHead,
                '--scenario',scenario,
                '--evidence',join(evidence,'kernel-root-owned-'+scenario)
              ],{
                timeout:180000,env,
                mustContain:['SEC06C_ROOT_OWNED_UNIT_KERNEL_PROTOTYPE_PASS=1',
                             'scenario='+scenario],
                mustReject:['SEC06C_ROOT_OWNED_BOUNDARY_BLOCKED']
              });
            }
            doCase('linux-original-host-sigkill','python3',[
              'benchmarks/e1/agent01l_cgroup_escape_crash_acceptance.py',
              '--guard',helper,'--parent',crashHost
            ],{
              env,timeout:180000,
              mustContain:['AGENT01L_NATIVE_CGROUP_ESCAPED_DESCENDANT_PASS=1',
                           'Lu_SIGKILL=1','original_scope_receipt=1']
            });
            doCase('linux-reviewed-git-original-journal','cargo',[
              'test','--locked','--manifest-path',
              'src-tauri/lu-core/Cargo.toml','--lib',
              'agent01l_linux_real_git_cgroup_custody_after_restart',
              '--','--ignored','--nocapture'
            ],{
              env,timeout:600000,
              mustContain:['agent01l_linux_real_git_cgroup_custody_after_restart',
                           '1 passed'],mustReject:['running 0 tests']
            });
            for(const modeName of [
              'guardian-death','guardian-stall','worker-stall','ancestor-migration'
            ]){
              doCase('linux-'+modeName,'python3',[
                'benchmarks/e1/p0_sec02_linux_guardian_negative.py',
                '--fixture',fixture,'--mode',modeName
              ],{
                timeout:180000,env,
                mustContain:[
                  modeName==='ancestor-migration'?
                    'P0_SEC05_NATIVE_ANCESTOR_MIGRATION_PASS=1':
                    'P0_SEC02_NATIVE_'+modeName.toUpperCase().replaceAll('-','_')+'_PASS=1'
                ],
                mustReject:['SEC06_BLOCKED_ANCESTOR_CGROUP_ESCAPE=1']
              });
            }
          }
        }
      }
    }else{
      doCase('windows-real-two-jobs','cargo',[
        'test','--locked','--manifest-path',
        'src-tauri/lu-core/Cargo.toml',
        '--lib','sec05_windows_two_jobs_cross_chat_cancel_cannot_terminate_foreign_worker',
        '--','--nocapture'
      ],{
        timeout:600000,
        mustContain:[
          'sec05_windows_two_jobs_cross_chat_cancel_cannot_terminate_foreign_worker',
          '1 passed'
        ],
        mustReject:['running 0 tests']
      });
      doCase('windows-reviewed-git-original-journal','cargo',[
        'test','--locked','--manifest-path',
        'src-tauri/lu-core/Cargo.toml','--lib',
        'agent01l_real_git_job_attests_original_turn_and_reopens_without_replay',
        '--','--nocapture'
      ],{
        timeout:600000,
        mustContain:[
          'agent01l_real_git_job_attests_original_turn_and_reopens_without_replay',
          '1 passed'
        ],
        mustReject:['running 0 tests']
      });
    }
    writeReceipt(mode);
  }
}
