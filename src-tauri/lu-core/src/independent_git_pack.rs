//! E1-P12: independent read-only Git pack/index v2 decoder.
//!
//! Bounded to eight <=64MiB PACK files and 64MiB per pack, <=100k objects,
//! <=2MiB decompressed objects and <=16 delta links. No Git invocation,
//! dynamic executable, external alternate store, repo mutation or network.
//! Verifies full PACK and IDX checksums, every requested materialized object
//! SHA1/SHA256 identity, index/pack counts, strict delta copy/insert bounds.
//! Invalid or unsupported pack features return None: never fabricate history.
use std::{collections::{BTreeMap,HashSet},fs,io::Read,path::Path};
use flate2::read::ZlibDecoder;
use sha2::{Digest,Sha256};
use crate::independent_git_history::{sha1_digest,git_hex};

const MAX_PACKS:usize=8;
const MAX_PACK_BYTES:usize=64*1024*1024;
const MAX_INDEX_BYTES:usize=16*1024*1024;
const MAX_OBJECTS:usize=100_000;
const MAX_OBJECT_BYTES:usize=2*1024*1024;
const MAX_DELTA_DEPTH:usize=16;
fn be32(b:&[u8])->Option<u32>{Some(u32::from_be_bytes(b.get(..4)?.try_into().ok()?))}
fn be64(b:&[u8])->Option<u64>{Some(u64::from_be_bytes(b.get(..8)?.try_into().ok()?))}
fn hex_hash(bytes:&[u8],width:usize)->Option<Vec<u8>>{
    let digested=if width==20{
        sha1_digest(bytes).to_vec()
    }else if width==32{
        Sha256::digest(bytes).to_vec()
    }else{return None};
    Some(digested)
}
fn safe_regular(path:&Path,max:usize)->Option<Vec<u8>>{
    for component in path.ancestors(){
        let info=fs::symlink_metadata(component).ok()?;
        if info.file_type().is_symlink(){return None}
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if info.file_attributes()&0x400!=0{return None}
        }
    }
    let info=fs::symlink_metadata(path).ok()?;
    if !info.is_file()||info.len()==0||info.len()>max as u64{return None}
    let mut data=Vec::new();
    fs::File::open(path).ok()?.take(max as u64+1)
        .read_to_end(&mut data).ok()?;
    if data.len()>max{return None}
    Some(data)
}
#[derive(Debug)]
struct PackFile{
    index:BTreeMap<String,u64>,
    offsets:BTreeMap<u64,String>,
    pack:Vec<u8>,
    hash_width:usize,
}
#[derive(Debug)]
pub struct PackCollection{
    packs:Vec<PackFile>,
    hash_width:usize,
}
impl PackCollection{
    pub fn open(git:&Path,hash_width:usize)->Option<Self>{
        if !matches!(hash_width,20|32){return None}
        let dir=git.join("objects").join("pack");
        if !dir.exists(){return Some(Self{packs:vec![],hash_width})}
        let info=fs::symlink_metadata(&dir).ok()?;
        if !info.is_dir()||info.file_type().is_symlink(){return None}
        let mut names=fs::read_dir(&dir).ok()?.filter_map(Result::ok)
            .filter_map(|entry|{
                let name=entry.file_name().into_string().ok()?;
                name.ends_with(".idx").then_some(name)
            }).collect::<Vec<_>>();
        names.sort();
        if names.len()>MAX_PACKS{return None}
        let mut packs=Vec::new();
        for name in names{
            if !name.starts_with("pack-")
                ||!name[..name.len()-4].bytes().skip(5).all(|b|
                    b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){
                return None
            }
            let stem=name.strip_suffix(".idx")?;
            let index=safe_regular(&dir.join(&name),MAX_INDEX_BYTES)?;
            let pack=safe_regular(&dir.join(format!("{stem}.pack")),MAX_PACK_BYTES)?;
            packs.push(PackFile::decode(index,pack,hash_width)?);
        }
        Some(Self{packs,hash_width})
    }
    pub fn object(&self,oid:&str)->Option<(String,Vec<u8>)>{
        if oid.len()!=self.hash_width*2{return None}
        for pack in &self.packs{
            let Some(&offset)=pack.index.get(oid) else {continue};
            let mut visited=HashSet::new();
            let (ty,bytes)=pack.decode_object(offset,0,&mut visited)?;
            let name=match ty{1=>"commit",2=>"tree",3=>"blob",4=>"tag",_=>return None};
            let mut framed=format!("{name} {}\0",bytes.len()).into_bytes();
            framed.extend_from_slice(&bytes);
            if git_hex(&hex_hash(&framed,self.hash_width)?)!=oid{return None}
            return Some((name.into(),bytes))
        }
        None
    }
}
impl PackFile{
    fn decode(idx:Vec<u8>,pack:Vec<u8>,width:usize)->Option<Self>{
        if idx.len()<8+1024+width*2||idx.get(..4)?!=b"\xfftOc"
            ||be32(idx.get(4..8)?)?!=2
            ||pack.len()<12+width||pack.get(..4)?!=b"PACK"
            ||!matches!(be32(pack.get(4..8)?)?,2|3){
            return None
        }
        let mut fanout=0usize;
        for i in 0..256usize{
            let next=be32(idx.get(8+i*4..12+i*4)?)? as usize;
            if next<fanout||next>MAX_OBJECTS{return None}
            fanout=next;
        }
        let n=fanout;
        if be32(pack.get(8..12)?)? as usize!=n{return None}
        let ids_start=8+1024;
        let crcs_start=ids_start.checked_add(n.checked_mul(width)?)?;
        let offsets_start=crcs_start.checked_add(n.checked_mul(4)?)?;
        let extra_start=offsets_start.checked_add(n.checked_mul(4)?)?;
        if extra_start+width*2>idx.len(){return None}
        let raw_offsets=idx.get(offsets_start..extra_start)?;
        let high_count=raw_offsets.chunks_exact(4).filter(|x|
            be32(x).is_some_and(|n|n&0x8000_0000!=0)).count();
        let end=extra_start.checked_add(high_count.checked_mul(8)?)?;
        if end.checked_add(width*2)?!=idx.len(){return None}
        let pack_checksum=idx.get(end..end+width)?;
        let index_checksum=idx.get(end+width..end+2*width)?;
        if hex_hash(&idx[..idx.len()-width],width)?.as_slice()!=index_checksum
            ||pack.get(pack.len()-width..)?!=pack_checksum
            ||hex_hash(&pack[..pack.len()-width],width)?.as_slice()!=pack_checksum{
            return None
        }
        let mut index=BTreeMap::new();
        let mut reverse=BTreeMap::new();
        let mut last=None::<String>;
        for i in 0..n{
            let oid=git_hex(idx.get(ids_start+i*width..ids_start+(i+1)*width)?);
            if last.as_ref().is_some_and(|old|old>=&oid){return None}
            last=Some(oid.clone());
            let raw=be32(raw_offsets.get(i*4..i*4+4)?)?;
            let offset=if raw&0x8000_0000!=0{
                let ordinal=(raw&0x7fff_ffff) as usize;
                if ordinal>=high_count{return None}
                be64(idx.get(extra_start+ordinal*8..extra_start+ordinal*8+8)?)?
            }else{raw as u64};
            if offset<12||offset>=pack.len().checked_sub(width)? as u64
                ||reverse.insert(offset,oid.clone()).is_some(){
                return None
            }
            index.insert(oid,offset);
        }
        Some(Self{index,offsets:reverse,pack,hash_width:width})
    }
    fn decode_object(&self,offset:u64,depth:usize,visited:&mut HashSet<u64>)
        ->Option<(u8,Vec<u8>)>{
        if depth>MAX_DELTA_DEPTH||!visited.insert(offset){return None}
        let result=self.decode_entry(offset,depth,visited);
        visited.remove(&offset);
        result
    }
    fn decode_entry(&self,offset:u64,depth:usize,visited:&mut HashSet<u64>)
        ->Option<(u8,Vec<u8>)>{
        let end=self.pack.len().checked_sub(self.hash_width)?;
        let mut pos=usize::try_from(offset).ok()?;
        let mut first=*self.pack.get(pos)?;pos+=1;
        let kind=(first>>4)&7;
        let mut size=(first&0x0f) as usize;
        let mut shift=4usize;
        while first&0x80!=0{
            first=*self.pack.get(pos)?;pos+=1;
            size=size.checked_add(((first&0x7f) as usize).checked_shl(
                u32::try_from(shift).ok()?)?)?;
            shift=shift.checked_add(7)?;
            if shift>49{return None}
        }
        if size>MAX_OBJECT_BYTES||pos>=end{return None}
        let base=if kind==6{
            let mut byte=*self.pack.get(pos)?;pos+=1;
            let mut distance=(byte&0x7f) as u64;
            while byte&0x80!=0{
                byte=*self.pack.get(pos)?;pos+=1;
                distance=distance.checked_add(1)?
                    .checked_mul(128)?.checked_add((byte&0x7f) as u64)?;
            }
            let base=offset.checked_sub(distance)?;
            if base<12||base>=offset{return None}
            Some(base)
        }else if kind==7{
            let raw=self.pack.get(pos..pos+self.hash_width)?;
            pos=pos.checked_add(self.hash_width)?;
            let oid=git_hex(raw);
            Some(*self.index.get(&oid)?)
        }else{None};
        if pos>=end{return None}
        let mut decoder=ZlibDecoder::new(self.pack.get(pos..end)?);
        let mut decoded=Vec::new();
        decoder.by_ref().take(MAX_OBJECT_BYTES as u64+1)
            .read_to_end(&mut decoded).ok()?;
        if decoded.len()>MAX_OBJECT_BYTES||decoder.total_in()==0{return None}
        if let Some(base_offset)=base{
            let (base_kind,base_data)=self.decode_object(
                base_offset,depth+1,visited)?;
            let out=apply_delta(&base_data,&decoded,size)?;
            return Some((base_kind,out))
        }
        if !matches!(kind,1|2|3|4)||decoded.len()!=size{return None}
        Some((kind,decoded))
    }
}
fn read_delta_varint(bytes:&[u8],pos:&mut usize)->Option<usize>{
    let mut result=0usize;
    for shift in (0..=56).step_by(7){
        let byte=*bytes.get(*pos)?;
        *pos+=1;
        result=result.checked_add(
            ((byte&0x7f) as usize).checked_shl(shift)?)?;
        if byte&0x80==0{return Some(result)}
    }
    None
}
fn apply_delta(base:&[u8],delta:&[u8],expected:usize)->Option<Vec<u8>>{
    let mut pos=0usize;
    if read_delta_varint(delta,&mut pos)?!=base.len()
        ||read_delta_varint(delta,&mut pos)?!=expected
        ||expected>MAX_OBJECT_BYTES{return None}
    let mut out=Vec::with_capacity(expected);
    while pos<delta.len(){
        let instr=*delta.get(pos)?;pos+=1;
        if instr&0x80!=0{
            let mut offset=0usize;
            let mut count=0usize;
            for i in 0..4{
                if instr&(1<<i)!=0{
                    let part=(*delta.get(pos)? as usize).checked_shl(i*8)?;
                    offset=offset.checked_add(part)?;pos+=1;
                }
            }
            for i in 0..3{
                if instr&(1<<(i+4))!=0{
                    let part=(*delta.get(pos)? as usize).checked_shl(i*8)?;
                    count=count.checked_add(part)?;pos+=1;
                }
            }
            if count==0{count=65536}
            let end=offset.checked_add(count)?;
            out.extend_from_slice(base.get(offset..end)?);
        }else if instr!=0{
            let end=pos.checked_add(instr as usize)?;
            out.extend_from_slice(delta.get(pos..end)?);
            pos=end;
        }else{return None}
        if out.len()>expected{return None}
    }
    (out.len()==expected).then_some(out)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rejects_empty_invalid_and_corrupt_packs(){
        assert!(PackFile::decode(vec![],vec![],20).is_none());
        assert!(PackFile::decode(b"fake idx".to_vec(),b"fake pack".to_vec(),20).is_none());
        assert!(apply_delta(b"original",&[8,1,0],1).is_none());
    }
    #[test]
    fn bounded_delta_insert_and_copy_are_exact(){
        // Git delta format: base len 3, result len 6, copy 3 base bytes
        // then insert 'def'.
        let delta=[3,6,0x90,3,3,b'd',b'e',b'f'];
        assert_eq!(apply_delta(b"abc",&delta,6),Some(b"abcdef".to_vec()));
        assert!(apply_delta(b"abc",&delta,7).is_none());
    }
}
