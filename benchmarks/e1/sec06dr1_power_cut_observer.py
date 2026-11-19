#!/usr/bin/env python3
"""Disposable VM administrator: verify real live scanner and retained receipt."""
import pathlib,sqlite3,os,pwd,hashlib,sys,time,signal,subprocess,json,stat
p=pathlib.Path('/home/luprobe/sec06dr1'); r=pathlib.Path('/var/lib/lu-sec06c/native-receipts'); run=pathlib.Path('/run/lu-sec06c'); exe='/usr/libexec/lu/lu-file-scan-worker-acceptance'
def rows():
    with sqlite3.connect(p/'tasks.sqlite') as c:
        return c.execute('SELECT t.task_id,t.state,a.nonce,u.original_uid,a.chat,a.turn,a.kind,a.binary_sha256,a.argv_sha256 FROM lu_tasks t JOIN lu_task_linux_os_owned_native_admission a ON a.task_id=t.task_id JOIN lu_task_native_prelaunch_uid u ON u.task_id=t.task_id ORDER BY t.task_id').fetchall()
def digest(x):return hashlib.sha256(x.read_bytes()).hexdigest()
def rootfile(x):
    st=x.lstat(); assert stat.S_ISREG(st.st_mode) and st.st_uid==0 and st.st_nlink==1 and not st.st_mode&0o022
if sys.argv[1]=='delay-prepare':
    # Independently exercise delayed receipt visibility on a consistent backup
    # of the real post-crash database. The original receipt is moved, not edited.
    late=p/'late';late.mkdir(mode=0o700);uid=pwd.getpwnam('luprobe');os.chown(late,uid.pw_uid,uid.pw_gid)
    with sqlite3.connect(p/'tasks.sqlite') as source, sqlite3.connect(late/'tasks.sqlite') as dest:source.backup(dest)
    os.chown(late/'tasks.sqlite',uid.pw_uid,uid.pw_gid)
    receipt=r/(rows()[0][2]+'.receipt');rootfile(receipt);receipt.rename(r/(receipt.stem+'.held'))
    sys.exit(0)
if sys.argv[1]=='delay-restore':
    receipt=r/(rows()[0][2]+'.receipt');held=r/(receipt.stem+'.held');rootfile(held)
    assert not receipt.exists();held.rename(receipt)
    prior=json.loads(pathlib.Path('/root/sec06dr1-baseline').read_text());assert digest(receipt)==prior['receipt']
    sys.exit(0)
if sys.argv[1]=='prepare':
    deadline=time.monotonic()+80
    while not (p/'power-ready').is_file():
        assert time.monotonic()<deadline,'no live progress';time.sleep(.02)
    stopped,live=rows();assert stopped[:2]==('scan-'+'b'*32,'RUNNING') and live[:2]==('scan-'+'c'*32,'RUNNING')
    assert all(x[3]==pwd.getpwnam('luprobe').pw_uid for x in (stopped,live))
    receipt=r/(stopped[2]+'.receipt');rootfile(receipt);assert receipt.read_text().endswith('KERNEL_EMPTY=1\nEFFECT=INTERRUPTED\n')
    assert not (r/(live[2]+'.receipt')).exists()
    for row in (stopped,live):rootfile(r/(row[2]+'.spent'))
    marker=run/(live[2]+'.go');rootfile(marker)
    leaf=pathlib.Path('/sys/fs/cgroup/system.slice')/f'lu-sec06c-native-{live[2]}.service'/f'lu-sec06c-leaf-{live[2]}'
    members=[int(x) for x in (leaf/'cgroup.procs').read_text().split()];assert len(members)==2 and leaf.stat().st_uid==0
    worker=next(x for x in members if os.readlink(f'/proc/{x}/exe')==exe)
    fd=os.pidfd_open(worker);signal.pidfd_send_signal(fd,signal.SIGSTOP)
    deadline=time.monotonic()+3
    while pathlib.Path(f'/proc/{worker}/stat').read_text().rsplit(')',1)[1].split()[0] not in ('T','t'):
        assert time.monotonic()<deadline;time.sleep(.01)
    assert (leaf/'cgroup.events').read_text().startswith('populated 1\n')
    # Persist only the observer baseline; no shutdown or cleanup request.
    pathlib.Path('/root/sec06dr1-baseline').write_text(json.dumps({'boot':pathlib.Path('/proc/sys/kernel/random/boot_id').read_text(),'receipt':digest(receipt),'spent':[digest(r/(x[2]+'.spent')) for x in (stopped,live)]}))
    os.sync();print('SEC06DR1_ACTIVE_POWER_CUT_READY',flush=True)
    while True:time.sleep(1)
else:
    stopped,live=rows(); prior=json.loads(pathlib.Path('/root/sec06dr1-baseline').read_text())
    assert pathlib.Path('/proc/sys/kernel/random/boot_id').read_text()!=prior['boot']
    assert digest(r/(stopped[2]+'.receipt'))==prior['receipt']
    assert not (r/(live[2]+'.receipt')).exists()
    assert [digest(r/(x[2]+'.spent')) for x in (stopped,live)]==prior['spent']
    assert not (run/(live[2]+'.go')).exists()
    assert not list(pathlib.Path('/sys/fs/cgroup/system.slice').glob('lu-sec06c-native-*.service'))
    # Try exact old input under the original UID, no new grant. The durable
    # denial-only tombstone must block the historical nonce before systemd.
    cmd=['runuser','-u','luprobe','--','sudo','-n','/usr/libexec/lu/lu-sec06c-launch-native']
    for k,v in zip(('nonce','chat','turn','call','kind','binary','binary-sha','args-sha'),(live[2],live[4],live[5],live[0],live[6],exe,live[7],live[8])):cmd += ['--'+k,v]
    cmd += ['--arg='+x for x in json.loads((p/'live-argv.json').read_text())]
    result=subprocess.run(cmd,stdin=subprocess.DEVNULL,capture_output=True,timeout=10)
    assert result.returncode!=0 and b'persistent original native nonce already consumed' in result.stderr, result.stderr
    assert not (r/(live[2]+'.receipt')).exists()
    assert not list(pathlib.Path('/sys/fs/cgroup/system.slice').glob('lu-sec06c-native-*.service'))
    print('SEC06DR1_STALE_MARKER_GONE_DURABLE_REPLAY_DENIED_PASS',flush=True)
