//! Native Unix ar and Debian binary .deb package reader.
//! Archive contents are inspectable data, NEVER installed or executed.
//! Supports common ar (short, GNU name table, BSD #1 extended names).
//! Debian 2.x is parsed as exact debian-binary/control.tar/data.tar ordering;
//! the two nested TARs are exposed under control/ and data/ to prevent
//! accidental system-root publication. Existing paths are never replaced.
use std::{collections::BTreeSet,io::Read};
use cap_std::fs::Dir;
use crate::native_linux_archive::{linux_archive_tree_members_from_bytes,LinuxArchiveError};
const MAX_INPUT:u64=128*1024*1024;
const MAX_MEMBER:usize=64*1024*1024;
const MAX_TOTAL:usize=256*1024*1024;
const MAX_MEMBERS:usize=4096;
const HEADER:usize=60;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ArError{
    UnsupportedFormat,UnsafeSource,InvalidHeader,UnsafeName,
    DuplicateName,TooLarge,MissingMember,UnsupportedNested,
    Io(String),
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ArEntry{pub name:String,pub data:Vec<u8>}
pub fn is_ar_name(name:&str)->bool{
    let n=name.to_ascii_lowercase();
    n.ends_with(".a")||n.ends_with(".ar")||n.ends_with(".deb")
}
fn flat(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
        &&!s.starts_with(".lu-")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn number(data:&[u8])->Result<usize,ArError>{
    let text=std::str::from_utf8(data).map_err(|_|ArError::InvalidHeader)?.trim();
    if text.is_empty()||!text.bytes().all(|b|b.is_ascii_digit()){
        return Err(ArError::InvalidHeader)
    }
    text.parse::<usize>().map_err(|_|ArError::TooLarge)
}
fn gnu_table_name(table:&[u8],offset:usize)->Result<String,ArError>{
    if offset>=table.len(){return Err(ArError::InvalidHeader)}
    let trailing=&table[offset..];
    let end=trailing.iter().position(|b|*b==b'\n')
        .ok_or(ArError::InvalidHeader)?;
    let name=trailing[..end].strip_suffix(b"/").unwrap_or(&trailing[..end]);
    std::str::from_utf8(name).map(str::to_owned).map_err(|_|ArError::InvalidHeader)
}
pub fn parse_ar(raw:&[u8])->Result<Vec<ArEntry>,ArError>{
    if raw.len() as u64>MAX_INPUT{return Err(ArError::TooLarge)}
    if !raw.starts_with(b"!<arch>\n"){return Err(ArError::UnsupportedFormat)}
    let mut pos=8usize;
    let mut table:Option<Vec<u8>>=None;
    let mut seen=BTreeSet::new();
    let mut out=Vec::new();
    let mut total=0usize;
    while pos<raw.len(){
        let hdr_end=pos.checked_add(HEADER).ok_or(ArError::TooLarge)?;
        if hdr_end>raw.len(){return Err(ArError::InvalidHeader)}
        let hdr=&raw[pos..hdr_end];
        if hdr[58]!=96||hdr[59]!=b'\n'{return Err(ArError::InvalidHeader)}
        let field=std::str::from_utf8(&hdr[..16])
            .map_err(|_|ArError::InvalidHeader)?.trim();
        let size=number(&hdr[48..58])?;
        if size>MAX_INPUT as usize{return Err(ArError::TooLarge)}
        let end=hdr_end.checked_add(size).ok_or(ArError::TooLarge)?;
        let next=end.checked_add(size%2).ok_or(ArError::TooLarge)?;
        if next>raw.len(){return Err(ArError::InvalidHeader)}
        if size%2==1&&raw[end]!=b'\n'{
            return Err(ArError::InvalidHeader)
        }
        let body=&raw[hdr_end..end];
        if field=="//"{
            if table.is_some(){return Err(ArError::DuplicateName)}
            table=Some(body.to_vec());
        }else if field!="/"&&field!="__.SYMDEF"&&field!="__.SYMDEF SORTED"{
            let (name,data)=if let Some(len)=field.strip_prefix("#1/"){
                let prefix=number(len.as_bytes())?;
                if prefix==0||prefix>1024||prefix>body.len(){
                    return Err(ArError::InvalidHeader)
                }
                let name=std::str::from_utf8(&body[..prefix])
                    .map_err(|_|ArError::InvalidHeader)?
                    .trim_end_matches('\0').to_owned();
                (name,&body[prefix..])
            }else if field.starts_with('/')&&field.len()>1
                &&field[1..].bytes().all(|b|b.is_ascii_digit()){
                let offset=number(field[1..].as_bytes())?;
                (gnu_table_name(table.as_deref().ok_or(ArError::InvalidHeader)?,offset)?,body)
            }else{
                (field.trim_end_matches('/').to_owned(),body)
            };
            if !flat(&name){return Err(ArError::UnsafeName)}
            if out.len()>=MAX_MEMBERS||data.len()>MAX_MEMBER{
                return Err(ArError::TooLarge)
            }
            total=total.checked_add(data.len()).ok_or(ArError::TooLarge)?;
            if total>MAX_TOTAL{return Err(ArError::TooLarge)}
            if !seen.insert(name.to_lowercase()){return Err(ArError::DuplicateName)}
            out.push(ArEntry{name,data:data.to_vec()});
        }
        pos=next;
    }
    if out.is_empty(){return Err(ArError::MissingMember)}
    Ok(out)
}
fn safe_virtual_path(name:&str)->bool{
    !name.is_empty()&&name.len()<=1024&&!name.starts_with('/')&&
    name.split('/').count()<=16 &&
    name.split('/').all(|s|!s.is_empty()&&s!="."&&s!=".."
        &&s.len()<=200&&!s.starts_with(".lu-")&&!s.starts_with('-')
        &&!s.eq_ignore_ascii_case(".git")&&!s.ends_with('.')&&!s.ends_with(' ')
        &&!s.chars().any(|c|c.is_control()||matches!(c,'\\'|':')))
}
fn package_tar(entry:&ArEntry)->Result<Vec<(String,bool,Vec<u8>)>,ArError>{
    let allowed=[".tar",".tar.gz",".tar.xz",".tar.bz2",".tar.zst"];
    if !allowed.iter().any(|suffix|entry.name.ends_with(suffix)){
        return Err(ArError::UnsupportedNested)
    }
    linux_archive_tree_members_from_bytes(&entry.name,&entry.data)
        .map_err(|e|match e{
            LinuxArchiveError::TooLarge=>ArError::TooLarge,
            _=>ArError::UnsupportedNested
        })
}
/// Extract both data and control as inert files under distinct prefixes.
/// A Debian package is never installed and its scripts are never executed.
pub fn deb_virtual_members(raw:&[u8])->Result<Vec<(String,bool,Vec<u8>)>,ArError>{
    let list=parse_ar(raw)?;
    if list.len()<3||list[0].name!="debian-binary"{
        return Err(ArError::MissingMember)
    }
    let version=list[0].data.split(|b|*b==b'\n').next()
        .ok_or(ArError::InvalidHeader)?;
    if version.len()<3||!version.starts_with(b"2.")||
        !version[2..].iter().all(u8::is_ascii_digit){
        return Err(ArError::UnsupportedFormat)
    }
    // Debian permits harmless _ extension records between required sections.
    // Required ordered records must still be unique; extra records after
    // data.tar are inert and are not extracted.
    let items=list.iter().skip(1).filter(|m|!m.name.starts_with('_'))
        .collect::<Vec<_>>();
    if items.len()<2||!items[0].name.starts_with("control.tar")
        ||!items[1].name.starts_with("data.tar"){
        return Err(ArError::MissingMember)
    }
    let mut out=Vec::new();let mut seen=BTreeSet::new();let mut total=0usize;
    for (folder,member) in [("control",items[0]),("data",items[1])]{
        for (path,directory,bytes) in package_tar(member)?{
            let name=format!("{folder}/{path}");
            if !safe_virtual_path(&name){return Err(ArError::UnsafeName)}
            if !seen.insert(name.to_lowercase()){return Err(ArError::DuplicateName)}
            if out.len()>=MAX_MEMBERS||bytes.len()>MAX_MEMBER{
                return Err(ArError::TooLarge)
            }
            total=total.checked_add(bytes.len()).ok_or(ArError::TooLarge)?;
            if total>MAX_TOTAL{return Err(ArError::TooLarge)}
            out.push((name,directory,bytes));
        }
    }
    if out.is_empty(){return Err(ArError::MissingMember)}
    Ok(out)
}
pub fn ar_tree_members(root:&Dir,name:&str)
    ->Result<Vec<(String,bool,Vec<u8>)>,ArError>{
    if !flat(name)||!is_ar_name(name){return Err(ArError::UnsafeSource)}
    let meta=root.symlink_metadata(name).map_err(|e|ArError::Io(e.to_string()))?;
    if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>MAX_INPUT{
        return Err(ArError::UnsafeSource)
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(ArError::UnsafeSource)}
    }
    let file=root.open(name).map_err(|e|ArError::Io(e.to_string()))?;
    let m=file.metadata().map_err(|e|ArError::Io(e.to_string()))?;
    if !m.is_file()||m.len()>MAX_INPUT{return Err(ArError::UnsafeSource)}
    let mut raw=Vec::new();
    file.take(MAX_INPUT+1).read_to_end(&mut raw)
        .map_err(|e|ArError::Io(e.to_string()))?;
    if raw.len() as u64>MAX_INPUT{return Err(ArError::TooLarge)}
    if name.to_ascii_lowercase().ends_with(".deb"){
        deb_virtual_members(&raw)
    }else{
        Ok(parse_ar(&raw)?.into_iter().map(|m|(m.name,false,m.data)).collect())
    }
}
