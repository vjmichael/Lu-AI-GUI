//! E1-P11/P12: independently verified loose and bounded packed commit DAG.
//!
//! No subprocesses, model-provided paths, Git hooks, permissions, or network.
//! This is a deliberately conservative independent oracle for a reviewed
//! repository's reachable commit count, not a replacement for git rev-list.
//! Linked object stores, shallow boundaries, replace/graft semantics,
 //! malformed/oversized packs and >4096 commits remain UNSUPPORTED.
//! Even when the reviewed Git application returns successfully, this oracle
//! will never claim VERIFIED without loading, digest-checking and walking each
//! reachable commit object, including all merge parents exactly once.
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path,PathBuf},
};
use flate2::read::ZlibDecoder;
use sha2::{Digest,Sha256};
use crate::independent_git_pack::PackCollection;

const MAX_COMMITS:usize=4096;
const MAX_COMPRESSED:u64=512*1024;
const MAX_INFLATED:usize=2*1024*1024;
const MAX_PARENTS_PER_COMMIT:usize=32;

/// A supported, fully verified, distinct reachable-commit count.
/// None MUST be surfaced as "UNSUPPORTED" by the reviewed-tool bridge.
pub fn loose_commit_count(git_dir:&Path,head:&str)->Option<u64>{
    if !object_id(head){return None}
    // Git's replace refs / grafts and shallow boundaries change rev-list's
    // logical graph. Refuse a comparison rather than misrepresenting it.
    for unsupported in ["shallow","info/grafts","objects/info/alternates"]{
        if git_dir.join(unsupported).exists(){return None}
    }
    if git_dir.join("refs/replace").exists(){
        if fs::read_dir(git_dir.join("refs/replace")).ok()?.next().is_some(){
            return None
        }
    }
    // Git replacement refs may be *packed*, not just loose files.
    let packed_refs=git_dir.join("packed-refs");
    if packed_refs.exists(){
        no_links(&packed_refs)?;
        let meta=fs::metadata(&packed_refs).ok()?;
        if !meta.is_file()||meta.len()>MAX_INFLATED as u64{return None}
        let packed=fs::read(&packed_refs).ok()?;
        if packed.windows(b" refs/replace/".len())
            .any(|w|w==b" refs/replace/"){
            return None
        }
    }
    let mut seen=HashSet::new();
    let mut reader=ObjectReader::new(git_dir);
    let mut pending=vec![head.to_string()];
    while let Some(id)=pending.pop(){
        if !seen.insert(id.clone()){continue}
        if seen.len()>MAX_COMMITS{return None}
        let (kind,raw)=reader.read(&id)?;
        if kind!="commit"{return None}
        for parent in commit_parents(&raw,&id)?{
            if !seen.contains(&parent){pending.push(parent)}
        }
        if pending.len()>MAX_COMMITS*MAX_PARENTS_PER_COMMIT{
            return None
        }
    }
    Some(seen.len() as u64)
}
fn object_id(value:&str)->bool{
    matches!(value.len(),40|64)&&value.bytes().all(|c|
        c.is_ascii_digit()||(b'a'..=b'f').contains(&c))
}
fn no_links(path:&Path)->Option<()>{
    for part in path.ancestors(){
        let info=fs::symlink_metadata(part).ok()?;
        if info.file_type().is_symlink(){return None}
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if info.file_attributes()&0x400!=0{return None}
        }
    }
    Some(())
}
pub struct ObjectReader{
    root:PathBuf,
    packs:Option<PackCollection>,
}
impl ObjectReader{
    pub fn new(root:&Path)->Self{
        Self{root:root.to_path_buf(),packs:None}
    }
    pub fn read(&mut self,oid:&str)->Option<(String,Vec<u8>)>{
        if !object_id(oid){return None}
        let location=self.root.join("objects").join(&oid[..2]).join(&oid[2..]);
        if fs::symlink_metadata(&location).is_ok(){
            return read_loose_object(&location,oid)
        }
        // Only missing loose objects may trigger a packed lookup. Corrupt
        // existing loose files cannot be silently bypassed with a pack.
        if self.packs.is_none(){
            self.packs=Some(PackCollection::open(&self.root,oid.len()/2)?)
        }
        self.packs.as_ref()?.object(oid)
    }
}
/// Temporary public single-object access for the direct Git index/tree
/// observer. Callers traversing many objects should reuse ObjectReader.
pub fn read_object_raw(root:&Path,oid:&str)->Option<(String,Vec<u8>)>{
    ObjectReader::new(root).read(oid)
}
fn read_loose_object(location:&Path,oid:&str)->Option<(String,Vec<u8>)>{
    no_links(location)?;
    let stat=fs::symlink_metadata(location).ok()?;
    if !stat.is_file()||stat.len()==0||stat.len()>MAX_COMPRESSED{return None}
    let mut compressed=Vec::with_capacity(stat.len() as usize);
    fs::File::open(location).ok()?
        .take(MAX_COMPRESSED+1).read_to_end(&mut compressed).ok()?;
    if compressed.len() as u64>MAX_COMPRESSED{return None}
    let mut decoded=Vec::new();
    let mut decompressor=ZlibDecoder::new(compressed.as_slice());
    decompressor.by_ref().take(MAX_INFLATED as u64+1)
        .read_to_end(&mut decoded).ok()?;
    if decoded.len()>MAX_INFLATED
        ||decompressor.total_in() as usize!=compressed.len(){return None}
    let digest=if oid.len()==64{
        format!("{:x}",Sha256::digest(&decoded))
    }else{git_hex(&sha1_digest(&decoded))};
    if digest!=oid{return None}
    let header_end=decoded.iter().position(|b|*b==0)?;
    let header=std::str::from_utf8(&decoded[..header_end]).ok()?;
    let (kind,length)=header.split_once(' ')?;
    if !matches!(kind,"commit"|"tree"|"blob"|"tag"){return None}
    let len=length.parse::<usize>().ok()?;
    if len==0||len!=decoded.len().checked_sub(header_end+1)?{
        return None
    }
    Some((kind.to_string(),decoded[header_end+1..].to_vec()))
}
fn commit_parents(body:&[u8],self_id:&str)->Option<Vec<String>>{
    let text=std::str::from_utf8(body).ok()?;
    let mut tree=0;
    let mut author=false;
    let mut committer=false;
    let mut parents=Vec::new();
    let mut parent_set=HashSet::new();
    let mut finished_headers=false;
    for line in text.lines(){
        if finished_headers{break}
        if line.is_empty(){finished_headers=true;break}
        if line.starts_with(' '){continue} // continuation (e.g. gpgsig)
        if let Some(oid)=line.strip_prefix("tree "){
            if !object_id(oid)||oid.len()!=self_id.len(){return None}
            tree+=1;
        }else if let Some(oid)=line.strip_prefix("parent "){
            if !object_id(oid)||oid.len()!=self_id.len()
                ||oid==self_id||!parent_set.insert(oid.to_string()){
                return None
            }
            parents.push(oid.to_string());
            if parents.len()>MAX_PARENTS_PER_COMMIT{return None}
        }else if line.starts_with("author "){author=true}
        else if line.starts_with("committer "){committer=true}
    }
    if !finished_headers||tree!=1||!author||!committer{return None}
    Some(parents)
}
pub(crate) fn git_hex(bytes:&[u8])->String{
    const HEX:&[u8;16]=b"0123456789abcdef";
    let mut out=String::with_capacity(bytes.len()*2);
    for b in bytes{
        out.push(HEX[(b>>4) as usize] as char);
        out.push(HEX[(b&15) as usize] as char);
    }
    out
}

/// SHA-1 matches Git's 40-hex object-ID encoding. This small independent
/// implementation is used for validating stored Git objects only, never
/// passwords, signatures or general-purpose cryptography.
pub(crate) fn sha1_digest(message:&[u8])->[u8;20]{
    let mut state:[u32;5]=[
        0x67452301,0xefcdab89,0x98badcfe,0x10325476,0xc3d2e1f0
    ];
    let bit_len=(message.len() as u64).wrapping_mul(8);
    let mut padded=Vec::with_capacity((message.len()+72+63)&!63);
    padded.extend_from_slice(message);
    padded.push(0x80);
    while padded.len()%64!=56{padded.push(0)}
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for block in padded.chunks_exact(64){
        let mut words=[0u32;80];
        for (i,chunk) in block.chunks_exact(4).enumerate(){
            words[i]=u32::from_be_bytes(chunk.try_into().expect("4 bytes"))
        }
        for i in 16..80{
            words[i]=(words[i-3]^words[i-8]^words[i-14]^words[i-16])
                .rotate_left(1);
        }
        let [mut a,mut b,mut c,mut d,mut e]=state;
        for (i,word) in words.iter().enumerate(){
            let (f,k)=match i{
                0..=19=>((b&c)|((!b)&d),0x5a827999),
                20..=39=>(b^c^d,0x6ed9eba1),
                40..=59=>((b&c)|(b&d)|(c&d),0x8f1bbcdc),
                _=>(b^c^d,0xca62c1d6)
            };
            let temp=a.rotate_left(5).wrapping_add(f)
                .wrapping_add(e).wrapping_add(k).wrapping_add(*word);
            e=d;d=c;c=b.rotate_left(30);b=a;a=temp;
        }
        for (acc,part) in state.iter_mut().zip([a,b,c,d,e]){
            *acc=acc.wrapping_add(part)
        }
    }
    let mut digest=[0u8;20];
    for (i,word) in state.iter().enumerate(){
        digest[i*4..i*4+4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(test)]
mod tests{
    use super::*;
    use flate2::{write::ZlibEncoder,Compression};
    use std::{io::Write,path::PathBuf,time::{SystemTime,UNIX_EPOCH}};
    fn root()->PathBuf{
        let p=std::env::temp_dir().join(format!(
            "lu-e1-p11-history-{}-{}",
            std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH)
                .expect("time").as_nanos()
        ));
        fs::create_dir_all(p.join("objects")).unwrap();
        p
    }
    fn add_commit(root:&Path,parents:&[&str])->String{
        let tree="a".repeat(40);
        let mut payload=format!("tree {tree}\n");
        for id in parents{payload.push_str(&format!("parent {id}\n"))}
        payload.push_str(
            "author Lu <lu@example.test> 1 +0000\n\
             committer Lu <lu@example.test> 1 +0000\n\n\
             synthetic native DAG\n");
        let raw=format!("commit {}\0{}",payload.len(),payload).into_bytes();
        let sha=git_git_hex(&sha1_digest(&raw));
        let folder=root.join("objects").join(&sha[..2]);
        fs::create_dir_all(&folder).unwrap();
        let mut writer=ZlibEncoder::new(Vec::new(),Compression::default());
        writer.write_all(&raw).unwrap();
        fs::write(folder.join(&sha[2..]),writer.finish().unwrap()).unwrap();
        sha
    }
    #[test]
    fn verified_sha1_matches_known_git_object_vectors(){
        assert_eq!(git_git_hex(&sha1_digest(b"")),"da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(git_git_hex(&sha1_digest(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
    #[test]
    fn independent_loose_commit_graph_counts_merges_without_double_counting(){
        let dir=root();
        let root_id=add_commit(&dir,&[]);
        let left=add_commit(&dir,&[&root_id]);
        let right=add_commit(&dir,&[&root_id]); // identical body would duplicate
        // Add distinct right commit so the DAG includes four object IDs.
        let right=if right==left{
            let second=add_commit(&dir,&[&left]); // linear fallback
            second
        }else{right};
        let merged=add_commit(&dir,&[&left,&right]);
        assert_eq!(loose_commit_count(&dir,&merged),Some(4));
        assert_eq!(loose_commit_count(&dir,&root_id),Some(1));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsupported_missing_packed_corrupt_and_malicious_objects_never_verify(){
        let dir=root();
        let head=add_commit(&dir,&[]);
        assert_eq!(loose_commit_count(&dir,&head),Some(1));
        fs::write(dir.join("shallow"),format!("{head}\n")).unwrap();
        assert_eq!(loose_commit_count(&dir,&head),None);
        fs::remove_file(dir.join("shallow")).unwrap();
        fs::write(dir.join("packed-refs"),
            format!("{head} refs/replace/1234567890123456789012345678901234567890\n")).unwrap();
        assert_eq!(loose_commit_count(&dir,&head),None);
        fs::remove_file(dir.join("packed-refs")).unwrap();
        fs::write(dir.join("objects").join(&head[..2]).join(&head[2..]),
            b"incorrect compressed object").unwrap();
        assert_eq!(loose_commit_count(&dir,&head),None);
        assert_eq!(loose_commit_count(&dir,"../../outside"),None);
        fs::remove_dir_all(dir).unwrap();
    }
}
