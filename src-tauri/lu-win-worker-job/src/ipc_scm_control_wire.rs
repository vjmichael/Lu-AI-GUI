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
pub const MAX_CONTROL_BODY:usize=3072;
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
    ReadNvpPublicEnrollment=5,
}
impl TryFrom<u8> for ScmCommand{
    type Error=Refusal;
    fn try_from(raw:u8)->Result<Self,Self::Error>{
        match raw{
            1=>Ok(Self::ReadStatus),
            2=>Ok(Self::LaunchFixedExecutor),
            3=>Ok(Self::StopExactExecutor),
            4=>Ok(Self::InspectStoppedExecutor),
            5=>Ok(Self::ReadNvpPublicEnrollment),
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
/// LU-IPC-GPU-01D-R2: immutable request binding for a future SCM-owned
/// NVP authentication broker. This is a canonical *scope*, NOT a credential,
/// signature, admission certificate, GPU grant, or evidence of worker stop.
/// The trusted SCM service must independently obtain and hold the kernel
/// process/image witnesses and verify its own credential before using it.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NvpBrokerScope{
    pub original:OriginalCall,
    pub nvp_pid:u32,
    pub nvp_birth_filetime:u64,
    pub nvp_session_id:u32,
    pub service_pid:u32,
    pub service_birth_filetime:u64,
    pub challenge:[u8;CHALLENGE_BYTES],
}
impl NvpBrokerScope{
    const DOMAIN:&'static [u8]=b"LU_NVP_SCM_SESSION_SCOPE_V1\0";
    /// A deterministic, tightly bounded scope for a native challenge/proof
    /// exchange. Never deserialize this from NVP-controlled JSON as authority.
    pub fn canonical(&self)->Result<Vec<u8>,Refusal>{
        self.original.checked()?;
        if self.original.module_id!="nvp.film" ||
            self.nvp_pid==0 || self.nvp_birth_filetime==0 ||
            self.nvp_session_id==0 || self.service_pid==0 ||
            self.service_birth_filetime==0 ||
            self.challenge.iter().all(|b|*b==0) {
            return Err(Refusal::InvalidScope)
        }
        let mut out=Vec::with_capacity(512);
        out.extend_from_slice(Self::DOMAIN);
        out.extend_from_slice(&self.challenge);
        for id in [self.nvp_pid,self.nvp_session_id,self.service_pid]{
            out.extend_from_slice(&id.to_le_bytes());
        }
        for birth in [self.nvp_birth_filetime,self.service_birth_filetime]{
            out.extend_from_slice(&birth.to_le_bytes());
        }
        for field in self.original.fields(){write_scope(&mut out,field)?;}
        if out.len()>MAX_CONTROL_BODY{return Err(Refusal::InvalidFrame)}
        Ok(out)
    }
    /// The trusted Core host must match the independently witnessed process
    /// generations and original operation before requesting SCM verification.
    /// This check grants no access without a separate authenticated broker.
    pub fn matches_host(
        &self,original:&OriginalCall,nvp_pid:u32,nvp_birth:u64,
        nvp_session:u32,service_pid:u32,service_birth:u64
    )->bool{
        self.original==*original &&
        self.nvp_pid==nvp_pid && self.nvp_birth_filetime==nvp_birth &&
        self.nvp_session_id==nvp_session &&
        self.service_pid==service_pid &&
        self.service_birth_filetime==service_birth &&
        self.canonical().is_ok()
    }
}


impl NvpBrokerScope{
    pub fn parse_exact(bytes:&[u8],service_pid:u32,service_birth:u64)
        ->Result<Self,Refusal>{
        if !bytes.starts_with(Self::DOMAIN) || bytes.len()>MAX_CONTROL_BODY {
            return Err(Refusal::InvalidFrame)
        }
        let mut at=Self::DOMAIN.len();
        let challenge:[u8;CHALLENGE_BYTES]=bytes.get(at..at+CHALLENGE_BYTES)
            .ok_or(Refusal::InvalidFrame)?.try_into()
            .map_err(|_|Refusal::InvalidFrame)?;
        at+=CHALLENGE_BYTES;
        fn u32_at(bytes:&[u8],at:&mut usize)->Result<u32,Refusal>{
            let value=u32::from_le_bytes(bytes.get(*at..*at+4)
                .ok_or(Refusal::InvalidFrame)?.try_into()
                .map_err(|_|Refusal::InvalidFrame)?);
            *at+=4;
            Ok(value)
        }
        fn u64_at(bytes:&[u8],at:&mut usize)->Result<u64,Refusal>{
            let value=u64::from_le_bytes(bytes.get(*at..*at+8)
                .ok_or(Refusal::InvalidFrame)?.try_into()
                .map_err(|_|Refusal::InvalidFrame)?);
            *at+=8;
            Ok(value)
        }
        let nvp_pid=u32_at(bytes,&mut at)?;
        let nvp_session_id=u32_at(bytes,&mut at)?;
        let observed_service=u32_at(bytes,&mut at)?;
        let nvp_birth_filetime=u64_at(bytes,&mut at)?;
        let observed_birth=u64_at(bytes,&mut at)?;
        if service_pid==0 || service_birth==0 ||
           observed_service!=service_pid || observed_birth!=service_birth{
            return Err(Refusal::StaleService)
        }
        let mut fields=Vec::with_capacity(MIN_FIELDS);
        for _ in 0..MIN_FIELDS {fields.push(read_scope(bytes,&mut at)?.to_owned());}
        if at!=bytes.len(){return Err(Refusal::InvalidFrame)}
        let scope=Self{
            original:OriginalCall{
                call_id:fields[0].clone(),fingerprint_sha256:fields[1].clone(),
                module_id:fields[2].clone(),session_ref:fields[3].clone(),
                core_epoch_ref:fields[4].clone(),conversation_ref:fields[5].clone(),
                user_turn_ref:fields[6].clone(),task_ref:fields[7].clone(),
                workspace_ref:fields[8].clone(),
            },nvp_pid,nvp_birth_filetime,nvp_session_id,
            service_pid:observed_service,service_birth_filetime:observed_birth,
            challenge,
        };
        if scope.canonical()?.as_slice()!=bytes{return Err(Refusal::InvalidFrame)}
        Ok(scope)
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
#[repr(u8)]
pub enum NvpAuthAction{Open=1,Check=2,Revoke=3}
impl TryFrom<u8> for NvpAuthAction{
    type Error=Refusal;
    fn try_from(value:u8)->Result<Self,Refusal>{
        match value{1=>Ok(Self::Open),2=>Ok(Self::Check),
            3=>Ok(Self::Revoke),_=>Err(Refusal::UnsupportedCommand)}
    }
}
/// A signed Core-to-SCM private request. The Hello and unsigned Welcome must
/// match the *actual* canonical lu.workspace.v0 frames observed by Core.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NvpAuthRequest{
    pub action:NvpAuthAction,
    pub scope:NvpBrokerScope,
    pub package_sha256:[u8;32],
    pub enrollment_sha256:[u8;32],
    pub image_sha256:[u8;32],
    pub signer_sha256:[u8;32],
    pub hello_wire:Vec<u8>,
    pub client_proof:[u8;32],
    pub welcome_without_mac:Vec<u8>,
    pub core_session:String,
}
impl NvpAuthRequest{
    pub const DOMAIN:&'static [u8]=b"LU_CORE_SCM_NVP_AUTH_V1\0";
    pub fn canonical(&self,scm_challenge:[u8;CHALLENGE_BYTES])
        ->Result<Vec<u8>,Refusal>{
        if scm_challenge.iter().all(|b|*b==0)||
           self.package_sha256.iter().all(|b|*b==0)||
           self.enrollment_sha256.iter().all(|b|*b==0)||
           self.image_sha256.iter().all(|b|*b==0)||
           self.signer_sha256.iter().all(|b|*b==0)||
           self.client_proof.iter().all(|b|*b==0)||
           self.hello_wire.is_empty()||self.hello_wire.len()>1024||
           self.welcome_without_mac.is_empty()||
           self.welcome_without_mac.len()>512 {
            return Err(Refusal::InvalidFrame)
        }
        let scope=self.scope.canonical()?;
        let mut body=Vec::with_capacity(1024);
        body.extend_from_slice(Self::DOMAIN);
        body.push(1); // exact private broker version
        body.push(self.action as u8);
        body.extend_from_slice(&scm_challenge);
        let size=u16::try_from(scope.len()).map_err(|_|Refusal::InvalidFrame)?;
        body.extend_from_slice(&size.to_le_bytes());
        body.extend_from_slice(&scope);
        body.extend_from_slice(&self.package_sha256);
        body.extend_from_slice(&self.enrollment_sha256);
        body.extend_from_slice(&self.image_sha256);
        body.extend_from_slice(&self.signer_sha256);
        body.extend_from_slice(&self.client_proof);
        for data in [&self.hello_wire,&self.welcome_without_mac]{
            let size=u16::try_from(data.len()).map_err(|_|Refusal::InvalidFrame)?;
            body.extend_from_slice(&size.to_le_bytes());
            body.extend_from_slice(data);
        }
        write_scope(&mut body,&self.core_session)?;
        if body.len()>MAX_CONTROL_BODY{return Err(Refusal::InvalidFrame)}
        Ok(body)
    }
    pub fn parse_exact(
        bytes:&[u8],scm_challenge:[u8;CHALLENGE_BYTES],
        service_pid:u32,service_birth:u64,
    )->Result<Self,Refusal>{
        if bytes.len()>MAX_CONTROL_BODY ||
           !bytes.starts_with(Self::DOMAIN){
            return Err(Refusal::InvalidFrame)
        }
        let mut at=Self::DOMAIN.len();
        if bytes.get(at)!=Some(&1){return Err(Refusal::InvalidFrame)}
        at+=1;
        let action=NvpAuthAction::try_from(
            *bytes.get(at).ok_or(Refusal::InvalidFrame)?)?;
        at+=1;
        if bytes.get(at..at+CHALLENGE_BYTES)!=Some(scm_challenge.as_slice()){
            return Err(Refusal::Replay)
        }
        at+=CHALLENGE_BYTES;
        fn take<'a>(bytes:&'a[u8],at:&mut usize,min:usize,max:usize)
            ->Result<&'a[u8],Refusal>{
            let size=u16::from_le_bytes(bytes.get(*at..*at+2)
                .ok_or(Refusal::InvalidFrame)?.try_into()
                .map_err(|_|Refusal::InvalidFrame)?) as usize;
            *at+=2;
            if size<min||size>max{return Err(Refusal::InvalidFrame)}
            let value=bytes.get(*at..*at+size).ok_or(Refusal::InvalidFrame)?;
            *at+=size;
            Ok(value)
        }
        let scope=NvpBrokerScope::parse_exact(
            take(bytes,&mut at,1,MAX_CONTROL_BODY)?,service_pid,service_birth)?;
        let read_hash=|at:&mut usize|->Result<[u8;32],Refusal>{
            let val=bytes.get(*at..*at+32).ok_or(Refusal::InvalidFrame)?;
            *at+=32;
            val.try_into().map_err(|_|Refusal::InvalidFrame)
        };
        let package_sha256=read_hash(&mut at)?;
        let enrollment_sha256=read_hash(&mut at)?;
        let image_sha256=read_hash(&mut at)?;
        let signer_sha256=read_hash(&mut at)?;
        let client_proof=read_hash(&mut at)?;
        let hello_wire=take(bytes,&mut at,1,1024)?.to_vec();
        let welcome_without_mac=take(bytes,&mut at,1,512)?.to_vec();
        let core_session=read_scope(bytes,&mut at)?.to_owned();
        if at!=bytes.len(){return Err(Refusal::InvalidFrame)}
        let request=Self{action,scope,package_sha256,enrollment_sha256,
            image_sha256,signer_sha256,
            hello_wire,client_proof,welcome_without_mac,core_session};
        if request.canonical(scm_challenge)?.as_slice()!=bytes{
            return Err(Refusal::InvalidFrame)
        }
        Ok(request)
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
    fn all_five_commands_have_exact_private_scopes(){
        for cmd in [ScmCommand::ReadStatus,ScmCommand::LaunchFixedExecutor,
            ScmCommand::StopExactExecutor,ScmCommand::InspectStoppedExecutor,
            ScmCommand::ReadNvpPublicEnrollment]{
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
    fn nvp_broker_scope_is_original_operation_and_process_generation_bound(){
        let scope=NvpBrokerScope{
            original:call(),nvp_pid:123,nvp_birth_filetime:456,
            nvp_session_id:7,service_pid:22,service_birth_filetime:33,
            challenge:[0x59;CHALLENGE_BYTES],
        };
        let canonical=scope.canonical().unwrap();
        assert!(canonical.starts_with(NvpBrokerScope::DOMAIN));
        assert!(scope.matches_host(&call(),123,456,7,22,33));
        assert!(!scope.matches_host(&call(),123,457,7,22,33));
        assert!(!scope.matches_host(&call(),123,456,7,22,34));
        assert!(!scope.matches_host(&call(),123,456,8,22,33));
        let mut wrong=scope.clone();
        wrong.original.user_turn_ref="another-turn".into();
        assert_ne!(wrong.canonical().unwrap(),canonical);
        assert!(!wrong.matches_host(&call(),123,456,7,22,33));
        wrong=scope.clone();
        wrong.original.module_id="lu.ide".into();
        assert_eq!(wrong.canonical(),Err(Refusal::InvalidScope));
        wrong=scope.clone();
        wrong.challenge=[0;CHALLENGE_BYTES];
        assert_eq!(wrong.canonical(),Err(Refusal::InvalidScope));
        wrong=scope.clone();
        wrong.nvp_birth_filetime=0;
        assert_eq!(wrong.canonical(),Err(Refusal::InvalidScope));
    }
    #[test]
    fn nvp_private_auth_is_canonical_scoped_and_replay_refusing(){
        let request=NvpAuthRequest{
            action:NvpAuthAction::Open,
            scope:NvpBrokerScope{
                original:call(),nvp_pid:123,nvp_birth_filetime:456,
                nvp_session_id:7,service_pid:42,service_birth_filetime:123,
                challenge:[0x42;32],
            },
            package_sha256:[0x33;32],enrollment_sha256:[0x44;32],
            image_sha256:[0x56;32],signer_sha256:[0x57;32],
            hello_wire:b"canonical hello".to_vec(),client_proof:[0x55;32],
            welcome_without_mac:b"unsigned welcome".to_vec(),
            core_session:"session".into(),
        };
        let challenge=[0x66;32];
        let bytes=request.canonical(challenge).unwrap();
        assert_eq!(NvpAuthRequest::parse_exact(&bytes,challenge,42,123).unwrap(),request);
        assert_eq!(NvpAuthRequest::parse_exact(&bytes,[0x67;32],42,123),
            Err(Refusal::Replay));
        assert_eq!(NvpAuthRequest::parse_exact(&bytes,challenge,43,123),
            Err(Refusal::StaleService));
        let mut wrong=request.clone();
        wrong.scope.original.user_turn_ref="other".into();
        assert_ne!(wrong.canonical(challenge).unwrap(),bytes);
        wrong=request.clone();wrong.scope.nvp_birth_filetime=0;
        assert_eq!(wrong.canonical(challenge),Err(Refusal::InvalidScope));
        wrong=request.clone();wrong.hello_wire=vec![1;1025];
        assert_eq!(wrong.canonical(challenge),Err(Refusal::InvalidFrame));
        for n in 0..bytes.len(){
            assert!(NvpAuthRequest::parse_exact(&bytes[..n],challenge,42,123).is_err());
        }
        let mut appended=bytes;appended.push(0);
        assert!(NvpAuthRequest::parse_exact(&appended,challenge,42,123).is_err());
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
