//! E1-P10: independent, bounded host observation of an approved resource.
//!
//! **Not** a repeat Git/FFprobe invocation, a model answer, or an OS process
//! termination test. The separate OracleEvidence is persisted only *after*
//! the original supervised worker effect reaches APPLIED with its receipt.
//! We currently verify direct Git HEAD/current-branch facts and ordinary
//! PCM/IEEE-float RIFF WAV, ordinary MP4, and simple tracked worktree
//! state. Nonconforming features remain UNSUPPORTED, never fabricated.
//!
//! No executable, path, permission, ID, codec hint or proof comes from model
//! text; the caller passes only current human-selected host-approved paths.
use std::{fs,io::Read,path::{Path,Component}};
use serde_json::Value;
use sha2::{Digest,Sha256};
use crate::{
    journal::IndependentOutcomeVerdict,
    independent_git_history::loose_commit_count,
    independent_git_status::tracked_worktree_snapshot,
    independent_mp4::{parse_mp4,matches_ffprobe},
    native_app_typed_git::GitReadOperation,
    native_app_typed_ffprobe::ReviewedContainer,
};
const MAX_HEAD:usize=4096;
const MAX_PACKED:usize=1024*1024;
const MAX_WAV:u64=256*1024*1024;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OracleEvidence{
    pub verdict:IndependentOutcomeVerdict,
    pub method:&'static str,
    pub observed_sha256:Option<String>,
}
impl OracleEvidence{
    fn verified(method:&'static str,observed:&[u8])->Self{
        Self{verdict:IndependentOutcomeVerdict::Verified,method,
            observed_sha256:Some(format!("{:x}",Sha256::digest(observed)))}
    }
    fn unsupported(method:&'static str)->Self{
        Self{verdict:IndependentOutcomeVerdict::Unsupported,method,
            observed_sha256:None}
    }
    fn conflict(method:&'static str)->Self{
        Self{verdict:IndependentOutcomeVerdict::Conflict,method,
            observed_sha256:None}
    }
}
fn is_hex_hash(text:&str)->bool{
    matches!(text.len(),40|64)&&text.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
fn safe_git_ref(reference:&str)->bool{
    let Some(branch)=reference.strip_prefix("refs/heads/")else{return false};
    if branch.is_empty()||branch.len()>128||branch.starts_with('-')
        ||branch.ends_with('/')||branch.ends_with(".lock")
        ||branch.contains("..")||branch.contains("//")
        ||branch.contains('@')||branch.contains('\\'){
        return false
    }
    branch.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.'|b'/'))
        &&Path::new(branch).components().all(|c|matches!(c,Component::Normal(_)))
}
fn read_nosymlink(path:&Path,max:usize)->Option<Vec<u8>>{
    // Check every existing component, not just the leaf. The trusted
    // Git catalog already prevents a linked repository root at admission.
    for current in path.ancestors(){
        let meta=fs::symlink_metadata(current).ok()?;
        if meta.file_type().is_symlink(){return None}
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes()&0x400!=0{return None}
        }
    }
    let meta=fs::symlink_metadata(path).ok()?;
    if !meta.is_file()||meta.len()>max as u64{return None}
    let mut file=fs::File::open(path).ok()?;
    let mut bytes=Vec::new();
    file.take(max as u64+1).read_to_end(&mut bytes).ok()?;
    if bytes.len()>max{return None}
    Some(bytes)
}
fn small_string(path:&Path,max:usize)->Option<String>{
    let bytes=read_nosymlink(path,max)?;
    let string=String::from_utf8(bytes).ok()?;
    let result=string.trim_end_matches(|c|c=='\r'||c=='\n');
    if result.chars().any(|c|matches!(c,'\r'|'\n'|'\0')){return None}
    Some(result.to_string())
}
enum DirectGitHead{
    Detached(String),
    Attached(String), // validated relative refs/heads/<branch>
}
fn direct_head(git_dir:&Path)->Option<DirectGitHead>{
    let contents=small_string(&git_dir.join("HEAD"),MAX_HEAD)?;
    if is_hex_hash(&contents){return Some(DirectGitHead::Detached(contents))}
    let reference=contents.strip_prefix("ref: ")?;
    if !safe_git_ref(reference){return None}
    Some(DirectGitHead::Attached(reference.to_string()))
}
fn resolve_git_ref(git_dir:&Path,reference:&str)->Option<String>{
    if !safe_git_ref(reference){return None}
    // A missing loose ref is expected after git pack-refs.
    let loose=git_dir.join(reference);
    if loose.exists(){
        let content=small_string(&loose,MAX_HEAD)?;
        return is_hex_hash(&content).then_some(content)
    }
    // packed-refs is a multiline Git text file. Unlike a single loose
    // ref, its *internal newlines* are legal and necessary; the previous
    // small_string() reader incorrectly rejected valid packed refs.
    let packed=String::from_utf8(
        read_nosymlink(&git_dir.join("packed-refs"),MAX_PACKED)?
    ).ok()?;
    let mut matched=None;
    for line in packed.lines(){
        if line.starts_with('#')||line.starts_with('^')||line.trim().is_empty(){
            continue
        }
        let Some((hash,name))=line.split_once(' ') else {return None};
        if name==reference{
            if matched.is_some()||!is_hex_hash(hash){return None}
            matched=Some(hash.to_string());
        }
    }
    matched
}
/// A source-derived revision witness observed independently from the
/// reviewed Git subprocess. This function never accepts the worker's
/// reported facts and returns None for states the bounded source validators
/// cannot independently represent. Includes the *content* of ordinary
/// tracked files, not only their dirty-file count.
pub fn git_source_revision(root:&Path,operation:GitReadOperation)->Option<String>{
    let git=root.join(".git");
    if !git.is_dir(){return None}
    let head=direct_head(&git)?;
    let source=match operation{
        GitReadOperation::CurrentBranch=>{
            match head{
                DirectGitHead::Detached(_)=>"detached".to_string(),
                DirectGitHead::Attached(name)=>format!("branch:{name}"),
            }
        },
        GitReadOperation::HeadCommit=>{
            let value=match head{
                DirectGitHead::Detached(hash)=>hash,
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname)?,
            };
            format!("head:{value}")
        },
        GitReadOperation::TrackedStatus=>{
            let value=match head{
                DirectGitHead::Detached(hash)=>hash,
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname)?,
            };
            let (count,state)=tracked_worktree_snapshot(root,&value)?;
            format!("tracked:{value}:{count}:{state}")
        },
        GitReadOperation::HistoryCount=>{
            let value=match head{
                DirectGitHead::Detached(hash)=>hash,
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname)?,
            };
            let count=loose_commit_count(&git,&value)?;
            format!("history:{value}:{count}")
        },
    };
    Some(format!("{:x}",Sha256::digest(source.as_bytes())))
}
/// Independently inspect the HEAD/branch files, without invoking Git or
/// reading stdout. A changed/malformed direct HEAD conflicts with the
/// worker receipt. Source observers operate without spawning Git; status
/// and pack traversal are strictly bounded and reject unsupported states.
pub fn verify_git(
    root:&Path,operation:GitReadOperation,worker_receipt:&Value,
)->OracleEvidence{
    let method=match operation{
        GitReadOperation::HeadCommit=>"GIT_HEAD_DIRECT_V1",
        GitReadOperation::CurrentBranch=>"GIT_BRANCH_DIRECT_V1",
        GitReadOperation::TrackedStatus=>"GIT_STATUS_INDEX_TREE_V1",
        GitReadOperation::HistoryCount=>"GIT_HISTORY_OBJECT_DAG_V2",
    };
    let git=root.join(".git");
    if !git.is_dir(){return OracleEvidence::conflict(method)}
    let Some(head)=direct_head(&git) else{
        return OracleEvidence::conflict(method)
    };
    match operation{
        GitReadOperation::TrackedStatus=>{
            let resolved=match head{
                DirectGitHead::Detached(hash)=>Some(hash),
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname),
            };
            let Some(hash)=resolved else{return OracleEvidence::unsupported(method)};
            let Some((changed,_content_revision))=
                tracked_worktree_snapshot(root,&hash) else{
                return OracleEvidence::unsupported(method)
            };
            if worker_receipt.get("kind").and_then(Value::as_str)
                !=Some("tracked_status")
                ||worker_receipt.get("trackedChangedEntries").and_then(Value::as_u64)
                    !=Some(changed as u64){
                return OracleEvidence::conflict(method)
            }
            OracleEvidence::verified(method,format!(
                "tracked={hash}:{changed}").as_bytes())
        },
        GitReadOperation::HistoryCount=>{
            let resolved=match head{
                DirectGitHead::Detached(hash)=>Some(hash),
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname),
            };
            let Some(hash)=resolved else{
                return OracleEvidence::unsupported(method)
            };
            // Directly decode, hash-check and traverse reachable
            // loose OR bounded packed objects; never invoke rev-list.
            let Some(actual)=loose_commit_count(&git,&hash) else{
                return OracleEvidence::unsupported(method)
            };
            if worker_receipt.get("kind").and_then(Value::as_str)
                !=Some("history_count")
                ||worker_receipt.get("historyCommitCount").and_then(Value::as_u64)
                    !=Some(actual){
                return OracleEvidence::conflict(method)
            }
            OracleEvidence::verified(method,
                format!("history={hash}:{actual}").as_bytes())
        },
        GitReadOperation::HeadCommit=>{
            let observed=match head{
                DirectGitHead::Detached(hash)=>Some(hash),
                DirectGitHead::Attached(refname)=>resolve_git_ref(&git,&refname),
            };
            let Some(hash)=observed else{
                return OracleEvidence::conflict(method)
            };
            if worker_receipt.get("kind").and_then(Value::as_str)
                !=Some("head_commit")
                ||worker_receipt.get("headCommit").and_then(Value::as_str)
                    !=Some(hash.as_str()){
                return OracleEvidence::conflict(method)
            }
            OracleEvidence::verified(method,format!("head={hash}").as_bytes())
        },
        GitReadOperation::CurrentBranch=>{
            let expected:Option<String>=match head{
                DirectGitHead::Detached(_)=>None,
                DirectGitHead::Attached(name)=>name.strip_prefix("refs/heads/")
                    .map(str::to_owned),
            };
            if worker_receipt.get("kind").and_then(Value::as_str)
                !=Some("current_branch")
                ||worker_receipt.get("currentBranch")
                    !=Some(&expected.as_ref().map_or(Value::Null,|b|
                        Value::String(b.clone()))){
                return OracleEvidence::conflict(method)
            }
            let identity=format!("branch={}",expected.as_deref().unwrap_or("(detached)"));
            OracleEvidence::verified(method,identity.as_bytes())
        },
        _=>unreachable!(),
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
struct WavFacts{
    duration_millis:u64,
    rate:u64,
    channels:u64,
    codec:&'static str,
}
fn le16(bytes:&[u8])->Option<u16>{
    Some(u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?))
}
fn le32(bytes:&[u8])->Option<u32>{
    Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
}
fn parse_wav(bytes:&[u8])->Option<WavFacts>{
    if bytes.len()<44||&bytes[..4]!=&b"RIFF"[..]
        ||&bytes[8..12]!=&b"WAVE"[..]
        ||le32(bytes.get(4..8)?)? as u64+8!=bytes.len() as u64{
        return None
    }
    let mut pos=12usize;
    let mut fmt:Option<(u16,u64,u64,u64,u64)>=None;
    let mut data_bytes=None;
    while pos+8<=bytes.len(){
        let name=bytes.get(pos..pos+4)?;
        let length=le32(bytes.get(pos+4..pos+8)?)? as usize;
        pos=pos.checked_add(8)?;
        let end=pos.checked_add(length)?;
        let chunk=bytes.get(pos..end)?;
        if name==b"fmt ".as_slice(){
                if fmt.is_some()||chunk.len()<16{return None}
                let format=le16(chunk.get(0..2)?)?;
                let channels=le16(chunk.get(2..4)?)? as u64;
                let rate=le32(chunk.get(4..8)?)? as u64;
                let byte_rate=le32(chunk.get(8..12)?)? as u64;
                let align=le16(chunk.get(12..14)?)? as u64;
                let bits=le16(chunk.get(14..16)?)? as u64;
                if channels==0||channels>64||rate==0||rate>768_000
                    ||align==0||align!=channels.checked_mul(bits/8)?
                    ||bits%8!=0||byte_rate!=rate.checked_mul(align)?{
                    return None
                }
                fmt=Some((format,rate,channels,align,bits));
        }else if name==b"data".as_slice(){
                if data_bytes.is_some()||length==0{return None}
                data_bytes=Some(length as u64);
        }
        pos=end.checked_add(length&1)?;
    }
    if pos!=bytes.len(){return None}
    let (format,rate,channels,align,bits)=fmt?;
    let data=data_bytes?;
    if data%align!=0{return None}
    let codec=match (format,bits){
        (1,8)=>"pcm_u8",
        (1,16)=>"pcm_s16le",
        (1,24)=>"pcm_s24le",
        (1,32)=>"pcm_s32le",
        (3,32)=>"pcm_f32le",
        (3,64)=>"pcm_f64le",
        _=>return None,
    };
    let duration_millis=(data as u128).checked_mul(1000)?
        .checked_add((align*rate/2) as u128)?
        .checked_div((align*rate) as u128)?;
    Some(WavFacts{
        duration_millis:u64::try_from(duration_millis).ok()?,
        rate,channels,codec
    })
}
/// Independently re-read the host-approved WAV, check its full SHA-256 and
/// parse RIFF/fmt/data ourselves. The same FFprobe binary is never invoked.
/// Only simple PCM/floating WAV and strictly parsed ordinary MP4 files can
/// qualify. Unsupported or mismatched media never becomes verified merely
/// because supervised FFprobe exited successfully.
pub fn verify_media(
    media:&Path,expected_sha:&str,kind:ReviewedContainer,worker_receipt:&Value,
)->OracleEvidence{
    let method=if kind==ReviewedContainer::Mp4{
        "MP4_BOXES_DIRECT_V1"
    }else{"WAV_PCM_DIRECT_V1"};
    let Some(meta)=fs::symlink_metadata(media).ok() else{
        return OracleEvidence::conflict(method)
    };
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_WAV{
        return OracleEvidence::conflict(method)
    }
    let Some(bytes)=read_nosymlink(media,MAX_WAV as usize) else{
        return OracleEvidence::conflict(method)
    };
    let observed_sha=format!("{:x}",Sha256::digest(&bytes));
    if observed_sha!=expected_sha||
        worker_receipt.get("approvedMediaSha256").and_then(Value::as_str)
            !=Some(expected_sha){
        return OracleEvidence::conflict(method)
    }
    if kind==ReviewedContainer::Mp4{
        let Some(movie)=parse_mp4(&bytes) else{
            return OracleEvidence::unsupported(method)
        };
        if !matches_ffprobe(&movie,worker_receipt){
            return OracleEvidence::conflict(method)
        }
        return OracleEvidence::verified(method,format!(
            "mp4={observed_sha}:{}:{:?}",movie.duration_millis,movie.tracks
        ).as_bytes())
    }
    let Some(wav)=parse_wav(&bytes) else{
        // Do not call compressed, extensible or unrecognized WAV independently
        // verified; an unparseable RIFF is still allowed only as UNSUPPORTED.
        return OracleEvidence::unsupported(method)
    };
    let Some(streams)=worker_receipt.get("streams").and_then(Value::as_array)
        else{return OracleEvidence::conflict(method)};
    if streams.len()!=1{return OracleEvidence::conflict(method)}
    let stream=&streams[0];
    let Some(duration)=worker_receipt.get("durationMilliseconds")
        .and_then(Value::as_u64) else{return OracleEvidence::conflict(method)};
    if duration.abs_diff(wav.duration_millis)>2
        ||stream.get("type").and_then(Value::as_str)!=Some("audio")
        ||stream.get("codec").and_then(Value::as_str)!=Some(wav.codec)
        ||stream.get("sampleRateHz").and_then(Value::as_u64)!=Some(wav.rate)
        ||stream.get("channels").and_then(Value::as_u64)!=Some(wav.channels){
        return OracleEvidence::conflict(method)
    }
    OracleEvidence::verified(method,
        format!("wav={}:{}:{}:{}:{}",
            observed_sha,wav.duration_millis,wav.rate,wav.channels,wav.codec)
                .as_bytes())
}

#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    use std::{time::{SystemTime,UNIX_EPOCH},path::PathBuf};
    fn root()->PathBuf{
        let path=std::env::temp_dir().join(format!(
            "lu-e1-p10-oracle-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn wav()->Vec<u8>{
        let count=8000u32;
        let mut b=Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36+count*2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&16000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(count*2).to_le_bytes());
        b.extend(vec![0u8;count as usize*2]);
        b
    }
    #[test]
    fn direct_git_head_ref_and_packed_ref_independent_of_executable(){
        let root=root();let git=root.join(".git");
        fs::create_dir_all(git.join("refs/heads")).unwrap();
        fs::write(git.join("HEAD"),"ref: refs/heads/main\n").unwrap();
        let hash="abcdef0123456789abcdef0123456789abcdef01";
        fs::write(git.join("refs/heads/main"),format!("{hash}\n")).unwrap();
        let received=json!({"kind":"head_commit","headCommit":hash});
        let pass=verify_git(&root,GitReadOperation::HeadCommit,&received);
        assert_eq!(pass.verdict,IndependentOutcomeVerdict::Verified);
        assert!(pass.observed_sha256.is_some());
        assert_eq!(verify_git(&root,GitReadOperation::CurrentBranch,
            &json!({"kind":"current_branch","currentBranch":"main"})).verdict,
            IndependentOutcomeVerdict::Verified);
        assert_eq!(verify_git(&root,GitReadOperation::HeadCommit,
            &json!({"kind":"head_commit","headCommit":"0000000000000000000000000000000000000000"}))
            .verdict,IndependentOutcomeVerdict::Conflict);
        fs::remove_file(git.join("refs/heads/main")).unwrap();
        fs::write(git.join("packed-refs"),format!("# pack-refs with: peeled\n{hash} refs/heads/main\n")).unwrap();
        assert_eq!(verify_git(&root,GitReadOperation::HeadCommit,&received).verdict,
            IndependentOutcomeVerdict::Verified);
        fs::write(git.join("HEAD"),format!("{hash}\n")).unwrap();
        assert_eq!(verify_git(&root,GitReadOperation::CurrentBranch,
            &json!({"kind":"current_branch","currentBranch":null})).verdict,
            IndependentOutcomeVerdict::Verified);
        assert_eq!(verify_git(&root,GitReadOperation::TrackedStatus,
            &json!({"trackedChangedEntries":0})).verdict,
            IndependentOutcomeVerdict::Unsupported);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn direct_wav_pcm_facts_and_digest_vs_false_ffprobe_claim(){
        let root=root();let file=root.join("approved.wav");
        fs::write(&file,wav()).unwrap();
        let sha=format!("{:x}",Sha256::digest(wav()));
        let actual=json!({
            "approvedMediaSha256":sha,
            "durationMilliseconds":1000,
            "streams":[{"type":"audio","codec":"pcm_s16le",
                "sampleRateHz":8000,"channels":1}]
        });
        assert_eq!(verify_media(&file,&sha,ReviewedContainer::Wave,&actual)
            .verdict,IndependentOutcomeVerdict::Verified);
        let mut forged=actual.clone();
        forged["streams"][0]["channels"]=json!(2);
        assert_eq!(verify_media(&file,&sha,ReviewedContainer::Wave,&forged)
            .verdict,IndependentOutcomeVerdict::Conflict);
        forged=actual.clone();
        forged["durationMilliseconds"]=json!(2500);
        assert_eq!(verify_media(&file,&sha,ReviewedContainer::Wave,&forged)
            .verdict,IndependentOutcomeVerdict::Conflict);
        fs::write(&file,b"modified after worker exit").unwrap();
        assert_eq!(verify_media(&file,&sha,ReviewedContainer::Wave,&actual)
            .verdict,IndependentOutcomeVerdict::Conflict);
        fs::write(&file,wav()).unwrap();
        assert_eq!(verify_media(&file,&sha,ReviewedContainer::Mp4,&actual)
            .verdict,IndependentOutcomeVerdict::Unsupported);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn history_requires_actual_loose_objects_not_a_git_stdout_claim(){
        let root=root();let git=root.join(".git");
        fs::create_dir_all(&git).unwrap();
        let hash="1234567890123456789012345678901234567890";
        fs::write(git.join("HEAD"),format!("{hash}\n")).unwrap();
        // HEAD syntax can be valid and the model/worker can report a count,
        // but absent independently verified objects MUST be unsupported.
        assert_eq!(verify_git(&root,GitReadOperation::HistoryCount,
            &json!({"kind":"history_count","historyCommitCount":1})).verdict,
            IndependentOutcomeVerdict::Unsupported);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hostile_git_ref_does_not_escape_approved_repository(){
        let root=root();fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/HEAD"),"ref: refs/heads/../../outside\n").unwrap();
        let check=verify_git(&root,GitReadOperation::HeadCommit,
            &json!({"kind":"head_commit","headCommit":"1234567890123456789012345678901234567890"}));
        assert_eq!(check.verdict,IndependentOutcomeVerdict::Conflict);
        fs::remove_dir_all(root).unwrap();
    }
}
