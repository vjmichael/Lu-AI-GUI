//! Private, bounded Lu Core -> installed SCM control wire (not the DLC gateway).
//!
//! This is a deterministic binary control envelope, deliberately separate from
//! the public lu.workspace.v0 Protobuf protocol. A signed request still does
//! NOT establish original user-turn authority; the Core dispatcher must commit
//! that grant before signing, and the SCM checks its own fixed installed
//! package, OS peer, revocation, nonce and service process generation.
//! Private control commands are never exposed through WorkspaceFrame.
use std::{convert::TryInto,fmt};

const DOMAIN:&[u8]=b"LU_CORE_SCM_CONTROL_V1\0";
pub const CHALLENGE_BYTES:usize=32;
pub const SIGNATURE_BYTES:usize=64;
pub const MAX_CONTROL_BODY:usize=1536;
pub const MAX_CONTROL_PACKET:usize=MAX_CONTROL_BODY+SIGNATURE_BYTES;
const MIN_FIELDS:usize=9;
const MAX_SCOPE:usize=128;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
#[repr(u8)]
pub enum ScmCommand{
    ReadStatus=1,
    LaunchFixedExecutor=2,
    StopExactExecutor=3,
    InspectStoppedExecutor=4,
}
impl TryFrom<u8> for ScmCommand{
    type Error=Refusal;
    fn try_from(raw:u8)->Result<Self,Self::Error>{
        match raw{
            1=>Ok(Self::ReadStatus),
            2=>Ok(Self::LaunchFixedExecutor),
            3=>Ok(Self::StopExactExecutor),
            4=>Ok(Self::InspectStoppedExecutor),
            _=>Err(Refusal::UnsupportedCommand),
        }
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Refusal{InvalidFrame,InvalidScope,StaleService,Replay,
    UnsupportedCommand,MissingAuthority}
impl fmt::Display for Refusal{
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{
        write!(f,"Lu SCM private control refused: {self:?}")
    }
}
impl std::error::Error for Refusal{}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct OriginalCall{
    pub call_id:String,
    pub fingerprint_sha256:String,
    pub module_id:String,
    pub session_ref:String,
    pub core_epoch_ref:String,
    pub conversation_ref:String,
    pub user_turn_ref:String,
    pub task_ref:String,
    pub workspace_ref:String,
}
impl OriginalCall{
    fn fields(&self)->[&str;MIN_FIELDS]{
        [&self.call_id,&self.fingerprint_sha256,&self.module_id,
         &self.session_ref,&self.core_epoch_ref,&self.conversation_ref,
         &self.user_turn_ref,&self.task_ref,&self.workspace_ref]
    }
    fn checked(&self)->Result<(),Refusal>{
        let h=|v:&str|v.len()==64&&v.bytes()
            .all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c));
        if !h(&self.call_id)||!h(&self.fingerprint_sha256){
            return Err(Refusal::InvalidScope)
        }
        for item in self.fields(){
            if item.is_empty()||item.len()>MAX_SCOPE||
                item.chars().any(char::is_control){
                return Err(Refusal::InvalidScope)
            }
        }
        Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ScmControl{
    pub command:ScmCommand,
    pub service_pid:u32,
    pub service_birth:u64,
    /// Caller must obtain from an actually running SCM witness and prove it
    /// on the same pipe; neither this number nor a saved PID is authority.
    pub original:Option<OriginalCall>,
}
fn write_scope(out:&mut Vec<u8>,v:&str)->Result<(),Refusal>{
    if v.is_empty()||v.len()>MAX_SCOPE||v.chars().any(char::is_control){
        return Err(Refusal::InvalidScope)
    }
    let size=u8::try_from(v.len()).map_err(|_|Refusal::InvalidScope)?;
    out.push(size);
    out.extend_from_slice(v.as_bytes());
    Ok(())
}
fn read_scope<'a>(raw:&'a[u8],at:&mut usize)->Result<&'a str,Refusal>{
    let size=*raw.get(*at).ok_or(Refusal::InvalidFrame)? as usize;
    *at+=1;
    if size==0||size>MAX_SCOPE{return Err(Refusal::InvalidScope)}
    let bytes=raw.get(*at..*at+size).ok_or(Refusal::InvalidFrame)?;
    *at+=size;
    let text=std::str::from_utf8(bytes).map_err(|_|Refusal::InvalidScope)?;
    if text.chars().any(char::is_control){return Err(Refusal::InvalidScope)}
    Ok(text)
}
impl ScmControl{
    pub fn canonical(&self,challenge:[u8;CHALLENGE_BYTES])
        ->Result<Vec<u8>,Refusal>{
        if self.service_pid==0||self.service_birth==0 ||
            challenge.iter().all(|v|*v==0){return Err(Refusal::InvalidFrame)}
        let has_call=self.command!=ScmCommand::ReadStatus;
        if has_call!=self.original.is_some(){return Err(Refusal::MissingAuthority)}
        let mut body=Vec::with_capacity(512);
        body.extend_from_slice(DOMAIN);
        body.push(1); // exact wire version
        body.push(self.command as u8);
        body.extend_from_slice(&challenge);
        body.extend_from_slice(&self.service_pid.to_le_bytes());
        body.extend_from_slice(&self.service_birth.to_le_bytes());
        if let Some(original)=&self.original{
            original.checked()?;
            for field in original.fields(){write_scope(&mut body,field)?;}
        }
        if body.len()>MAX_CONTROL_BODY{return Err(Refusal::InvalidFrame)}
        Ok(body)
    }
    pub fn parse_exact(
        bytes:&[u8],challenge:[u8;CHALLENGE_BYTES],
        service_pid:u32,service_birth:u64,
    )->Result<Self,Refusal>{
        if bytes.len()>MAX_CONTROL_BODY || bytes.len()<DOMAIN.len()+46 ||
            !bytes.starts_with(DOMAIN){
            return Err(Refusal::InvalidFrame)
        }
        let mut at=DOMAIN.len();
        if bytes[at]!=1{return Err(Refusal::InvalidFrame)}
        at+=1;
        let command=ScmCommand::try_from(bytes[at])?;
        at+=1;
        if bytes.get(at..at+32)!=Some(challenge.as_slice()){
            return Err(Refusal::Replay)
        }
        at+=32;
        let pid=u32::from_le_bytes(bytes.get(at..at+4)
            .ok_or(Refusal::InvalidFrame)?.try_into()
            .map_err(|_|Refusal::InvalidFrame)?);
        at+=4;
        let birth=u64::from_le_bytes(bytes.get(at..at+8)
            .ok_or(Refusal::InvalidFrame)?.try_into()
            .map_err(|_|Refusal::InvalidFrame)?);
        at+=8;
        if service_pid==0||service_birth==0||
           pid!=service_pid||birth!=service_birth {
            return Err(Refusal::StaleService)
        }
        let original=if command==ScmCommand::ReadStatus{
            None
        }else{
            let mut fields=Vec::with_capacity(MIN_FIELDS);
            for _ in 0..MIN_FIELDS {fields.push(read_scope(bytes,&mut at)?.to_owned());}
            let call=OriginalCall{
                call_id:fields[0].clone(),fingerprint_sha256:fields[1].clone(),
                module_id:fields[2].clone(),session_ref:fields[3].clone(),
                core_epoch_ref:fields[4].clone(),
                conversation_ref:fields[5].clone(),user_turn_ref:fields[6].clone(),
                task_ref:fields[7].clone(),workspace_ref:fields[8].clone(),
            };
            call.checked()?;
            Some(call)
        };
        if at!=bytes.len(){return Err(Refusal::InvalidFrame)}
        let result=Self{command,service_pid:pid,service_birth:birth,original};
        if result.canonical(challenge)?.as_slice()!=bytes{
            return Err(Refusal::InvalidFrame)
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn call()->OriginalCall{
        OriginalCall{
            call_id:"a".repeat(64),fingerprint_sha256:"b".repeat(64),
            module_id:"nvp.film".into(),session_ref:"sess".into(),
            core_epoch_ref:"epoch".into(),conversation_ref:"conv".into(),
            user_turn_ref:"turn".into(),task_ref:"task".into(),
            workspace_ref:"workspace".into(),
        }
    }
    fn signed(cmd:ScmCommand)->ScmControl{
        ScmControl{command:cmd,service_pid:42,service_birth:123,
            original:if cmd==ScmCommand::ReadStatus{None}else{Some(call())}}
    }
    #[test]
    fn all_four_commands_have_exact_private_scopes(){
        for cmd in [ScmCommand::ReadStatus,ScmCommand::LaunchFixedExecutor,
            ScmCommand::StopExactExecutor,ScmCommand::InspectStoppedExecutor]{
            let expected=signed(cmd);
            let challenge=[0x45;32];
            let raw=expected.canonical(challenge).unwrap();
            assert_eq!(ScmControl::parse_exact(&raw,challenge,42,123).unwrap(),expected);
            assert!(ScmControl::parse_exact(&raw,[0x46;32],42,123).is_err());
            assert!(ScmControl::parse_exact(&raw,challenge,43,123).is_err());
            assert!(ScmControl::parse_exact(&raw,challenge,42,124).is_err());
            let mut appended=raw.clone();appended.push(0);
            assert!(ScmControl::parse_exact(&appended,challenge,42,123).is_err());
        }
    }
    #[test]
    fn wrong_turn_lane_empty_scopes_and_fake_authority_refuse(){
        let mut wrong=signed(ScmCommand::StopExactExecutor);
        wrong.original.as_mut().unwrap().user_turn_ref="bad\nturn".into();
        assert_eq!(wrong.canonical([1;32]),Err(Refusal::InvalidScope));
        let mut invalid=signed(ScmCommand::LaunchFixedExecutor);
        invalid.original.as_mut().unwrap().call_id="A".repeat(64);
        assert_eq!(invalid.canonical([1;32]),Err(Refusal::InvalidScope));
        invalid.original=None;
        assert_eq!(invalid.canonical([1;32]),Err(Refusal::MissingAuthority));
        let raw=signed(ScmCommand::ReadStatus).canonical([1;32]).unwrap();
        let mut forbidden=raw.clone();forbidden[DOMAIN.len()+1]=0xff;
        assert_eq!(ScmControl::parse_exact(&forbidden,[1;32],42,123),
            Err(Refusal::UnsupportedCommand));
    }
    #[test]
    fn truncated_noncanonical_and_replayed_packets_are_refused(){
        let raw=signed(ScmCommand::LaunchFixedExecutor).canonical([1;32]).unwrap();
        for size in 0..raw.len(){
            assert!(ScmControl::parse_exact(&raw[..size],[1;32],42,123).is_err());
        }
        let mut padding=raw.clone();padding.extend_from_slice(&[0,0]);
        assert!(ScmControl::parse_exact(&padding,[1;32],42,123).is_err());
    }
}
