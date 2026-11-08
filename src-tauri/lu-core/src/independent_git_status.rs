//! E1-P12: bounded independent Git tracked-status count from the index,
//! committed HEAD tree and approved working files. NO Git executable.
//!
//! Verifies only a fully merged SHA-1 v2/v3 index whose entries exactly
//! match the current committed tree. Thus only *unstaged* changes to
//! ordinary tracked, regular 100644 files are counted (including deletion).
//! Staged additions/deletions/renames, worktree attributes/filters, symlinks,
//! sparse/split indexes, submodules and unusual index extensions are
//! UNSUPPORTED rather than guessed. A 0 count is not a general clean
//! repository guarantee unless every precondition below succeeded.
use std::{collections::{BTreeMap,BTreeSet},fs,io::Read,path::{Component,Path}};
use crate::independent_git_history::{sha1_digest,ObjectReader};
// Git's committed object identity is verified by the independent reader.

const MAX_INDEX:usize=4*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_FILE:u64=128*1024*1024;
const MAX_DEPTH:usize=16;
const MAX_TOTAL:u64=256*1024*1024;
fn be16(bytes:&[u8])->Option<u16>{
    Some(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?))
}
fn be32(bytes:&[u8])->Option<u32>{
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}
fn hex(bytes:&[u8])->String{
    const DIGITS:&[u8;16]=b"0123456789abcdef";
    let mut out=String::with_capacity(bytes.len()*2);
    for b in bytes{
        out.push(DIGITS[(b>>4) as usize] as char);
        out.push(DIGITS[(b&15) as usize] as char);
    }
    out
}
fn safe_path(path:&str)->bool{
    if path.is_empty()||path.len()>1024||path.chars().any(|ch|ch=='\0'
        ||ch=='\n'||ch=='\r'||ch.is_control())
        ||path.contains('\\')||path.starts_with('/'){return false}
    Path::new(path).components().all(|part|matches!(part,Component::Normal(_)))
}
fn no_links(path:&Path)->Option<()>{
    for part in path.ancestors(){
        let metadata=fs::symlink_metadata(part).ok()?;
        if metadata.file_type().is_symlink(){return None}
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes()&0x400!=0{return None}
        }
    }
    Some(())
}
fn index_entries(git:&Path)->Option<BTreeMap<String,String>>{
    let path=git.join("index");no_links(&path)?;
    let meta=fs::metadata(&path).ok()?;
    if !meta.is_file()||meta.len()>MAX_INDEX as u64{return None}
    let bytes=fs::read(path).ok()?;
    if bytes.len()<32||&bytes[..4]!=b"DIRC"{return None}
    let version=be32(bytes.get(4..8)?)?;
    if version!=2&&version!=3{return None}
    let count=be32(bytes.get(8..12)?)? as usize;
    if count>MAX_ENTRIES{return None}
    let data_end=bytes.len().checked_sub(20)?;
    if sha1_digest(&bytes[..data_end]).as_slice()!=&bytes[data_end..]{return None}
    let mut pos=12usize;
    let mut map=BTreeMap::new();
    for _ in 0..count{
        if pos.checked_add(62)?>data_end{return None}
        let mode=be32(bytes.get(pos+24..pos+28)?)?;
        // Only normal, non-executable files for now. Git's platform
        // executable-bit/filemode behavior would otherwise need checking.
        if mode!=0o100644{return None}
        let oid=hex(bytes.get(pos+40..pos+60)?);
        let flags=be16(bytes.get(pos+60..pos+62)?)?;
        if flags&0xf000!=0{return None} // no merge stages or extended flags
        let first=pos+62;
        let end=bytes.get(first..data_end)?.iter().position(|b|*b==0)?
            .checked_add(first)?;
        let name=std::str::from_utf8(bytes.get(first..end)?).ok()?;
        if !safe_path(name)||flags&0xfff!=0xfff
            &&(flags&0xfff) as usize!=name.len(){return None}
        let length=end.checked_add(1)?.checked_sub(pos)?;
        let aligned=(length+7)&!7;
        let next=pos.checked_add(aligned)?;
        if next>data_end||bytes.get(end+1..next)?.iter().any(|b|*b!=0){
            return None
        }
        if map.insert(name.to_string(),oid).is_some(){return None}
        pos=next;
    }
    while pos<data_end{
        let header=bytes.get(pos..pos+8)?;
        let extension=header.get(..4)?;
        let size=be32(header.get(4..8)?)? as usize;
        pos=pos.checked_add(8)?;
        let end=pos.checked_add(size)?;
        if end>data_end{return None}
        // TREE cache extension is merely an optimization; never trust
        // its hashes or contents in place of independent tree traversal.
        // Every other index extension is conservatively unsupported.
        if extension!=b"TREE"{return None}
        pos=end;
    }
    if pos!=data_end{return None}
    Some(map)
}
fn head_tree_id(commit:&[u8])->Option<String>{
    let text=std::str::from_utf8(commit).ok()?;
    let value=text.lines().next()?.strip_prefix("tree ")?;
    if value.len()!=40||!value.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return None}
    Some(value.to_string())
}
fn collect_tree(
    reader:&mut ObjectReader,oid:&str,prefix:&str,depth:usize,
    budget:&mut usize,entries:&mut BTreeMap<String,String>,
)->Option<()>{
    if depth>MAX_DEPTH||*budget==0{return None}
    let (kind,bytes)=reader.read(oid)?;
    if kind!="tree"{return None}
    let mut pos=0usize;
    while pos<bytes.len(){
        *budget=budget.checked_sub(1)?;
        let tail=bytes.get(pos..)?;
        let space=tail.iter().position(|b|*b==b' ')?;
        let mode=std::str::from_utf8(tail.get(..space)?).ok()?;
        let after_mode=pos+space+1;
        let zero=bytes.get(after_mode..)?.iter().position(|b|*b==0)?
            .checked_add(after_mode)?;
        let basename=std::str::from_utf8(bytes.get(after_mode..zero)?).ok()?;
        if !safe_path(basename)||basename.contains('/')||basename=="."||basename==".."{
            return None
        }
        let obj_start=zero+1;
        let obj_end=obj_start.checked_add(20)?;
        let child=hex(bytes.get(obj_start..obj_end)?);
        let name=if prefix.is_empty(){
            basename.to_string()
        }else{format!("{prefix}/{basename}")};
        match mode{
            "40000"|"040000"=>collect_tree(
                reader,&child,&name,depth+1,budget,entries
            )?,
            "100644"=>{
                if entries.insert(name,child).is_some(){return None}
            },
            _=>return None
        }
        if entries.len()>MAX_ENTRIES{return None}
        pos=obj_end;
    }
    Some(())
}
fn blob_oid(file:&[u8])->String{
    let mut value=format!("blob {}\0",file.len()).into_bytes();
    value.extend_from_slice(file);
    hex(&sha1_digest(&value))
}
/// Compute a semantic count ONLY if committed HEAD tree matches the actual
/// approved Git index exactly. This excludes staged changes and rename
/// detection, so each tracked path contributes at most one worktree change.
pub fn tracked_worktree_changes(repo:&Path,head:&str)->Option<u32>{
    let git=repo.join(".git");
    if head.len()!=40{return None} // SHA-256 indexes need a separate parser
    // Committed attributes or external clean/smudge filters can alter the
    // blob comparison and bypass an independent raw-byte status check.
    if repo.join(".gitattributes").exists()
        ||git.join("info/attributes").exists()
        ||git.join("modules").exists(){return None}
    let config=fs::read_to_string(git.join("config")).ok()?;
    let config=config.to_ascii_lowercase();
    if config.contains("autocrlf")||config.contains("eol")
        ||config.contains("filter ")||config.contains("sparsecheckout")
        ||config.contains("fsmonitor")||config.contains("ignorestat")
        ||config.contains("worktree"){return None}
    let index=index_entries(&git)?;
    let mut reader=ObjectReader::new(&git);
    let (kind,commit)=reader.read(head)?;
    if kind!="commit"{return None}
    let tree_id=head_tree_id(&commit)?;
    let mut tracked=BTreeMap::new();
    let mut budget=MAX_ENTRIES*4;
    collect_tree(&mut reader,&tree_id,"",0,&mut budget,&mut tracked)?;
    if index!=tracked{return None} // staged changes NOT claimed verified
    let mut changed=0u32;
    let mut bytes_read=0u64;
    for (name,expected) in tracked{
        let file=repo.join(&name);
        if !file.exists(){
            // Missing tracked file counts as exactly one porcelain entry.
            changed=changed.checked_add(1)?;
            continue
        }
        no_links(&file)?;
        let metadata=fs::symlink_metadata(&file).ok()?;
        if !metadata.is_file()||metadata.len()>MAX_FILE{return None}
        bytes_read=bytes_read.checked_add(metadata.len())?;
        if bytes_read>MAX_TOTAL{return None}
        let mut content=Vec::new();
        fs::File::open(&file).ok()?.take(MAX_FILE+1)
            .read_to_end(&mut content).ok()?;
        if content.len() as u64>MAX_FILE{return None}
        // Guard against EOL normalization, textconv, binary filters:
        // only simple byte-identical Linux/Windows files are supported.
        if content.contains(&b'\r'){return None}
        if blob_oid(&content)!=expected{
            changed=changed.checked_add(1)?;
        }
    }
    Some(changed)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn path_checks_and_hash_match_git_blob_convention(){
        assert!(!safe_path("../outside"));
        assert!(!safe_path("nested/../outside"));
        assert!(!safe_path("/etc/passwd"));
        assert!(safe_path("assets/tracked.txt"));
        assert_eq!(blob_oid(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a");
    }
    #[test]
    fn missing_index_or_malformed_head_never_claims_clean(){
        let dir=std::env::temp_dir().join(format!("lu-p12-status-{}",std::process::id()));
        fs::create_dir_all(dir.join(".git")).unwrap();
        assert_eq!(tracked_worktree_changes(&dir,
            "0000000000000000000000000000000000000000"),None);
        fs::remove_dir_all(dir).unwrap();
    }
}
