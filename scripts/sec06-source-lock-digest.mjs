// SEC-06F: source identity is the canonical committed Git blob, not checkout
// bytes that Git may transform between LF and CRLF on Windows and Linux.
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync,lstatSync} from 'node:fs';
import {join} from 'node:path';

export function canonicalSourceLockSha256(root){
  const lockPath=join(root,'source-tree.lock.json');
  if(!lstatSync(lockPath).isFile()){
    throw new Error('SEC06_SOURCE_LOCK_WORKTREE_MISMATCH: lock is not a regular file');
  }
  const disk=readFileSync(lockPath);
  const committed=execFileSync('git',[
    '-C',root,'cat-file','blob','HEAD:source-tree.lock.json'
  ],{encoding:'buffer',timeout:30000,maxBuffer:2*1024*1024,windowsHide:true});
  // The only permitted checkout transformation is CRLF -> LF. Git source
  // verification must still run independently, and no other edits normalize.
  const canonicalDisk=Buffer.from(disk.toString('utf8').replace(/\r\n/g,'\n'),'utf8');
  if(!canonicalDisk.equals(committed)){
    throw new Error('SEC06_SOURCE_LOCK_WORKTREE_MISMATCH: uncommitted lock bytes');
  }
  return createHash('sha256').update(committed).digest('hex');
}
