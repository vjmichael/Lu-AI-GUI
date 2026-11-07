use std::{fs,io::Write,sync::atomic::{AtomicU64,Ordering}};
use cap_std::{ambient_authority,fs::Dir};
use flate2::{write::GzEncoder,Compression};
use lu_core::{
    native_cpio::{parse_cpio_newc,cpio_tree_members,CpioError},
    native_ar_deb::{parse_ar,deb_virtual_members,ar_tree_members,ArError},
    native_archive_tree::{
        preview_archive_tree,extract_archive_tree_new,
        observe_tree_completion,TreeExtractError,
    },
};
static NEXT:AtomicU64=AtomicU64::new(0);
fn fixture()->(std::path::PathBuf,Dir){
    let dir=std::env::temp_dir().join(format!("lu-package-{}-{}",
        std::process::id(),NEXT.fetch_add(1,Ordering::SeqCst)));
    fs::create_dir_all(&dir).unwrap();
    let cap=Dir::open_ambient_dir(&dir,ambient_authority()).unwrap();
    (dir,cap)
}
fn hex(header:&mut Vec<u8>,value:u32){header.extend_from_slice(format!("{value:08x}").as_bytes())}
fn cpio_record(out:&mut Vec<u8>,name:&str,data:&[u8],mode:u32,crc:bool){
    let mut h=Vec::new();
    h.extend_from_slice(if crc{b"070702"}else{b"070701"});
    for value in [1,mode,0,0,1,0,data.len() as u32,0,0,0,0,
        (name.len()+1) as u32,
        if crc{data.iter().fold(0u32,|s,b|s.wrapping_add(*b as u32))}else{0}]{
        hex(&mut h,value);
    }
    assert_eq!(h.len(),110);
    out.extend_from_slice(&h);
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    while out.len()%4!=0{out.push(0)}
    out.extend_from_slice(data);
    while out.len()%4!=0{out.push(0)}
}
fn cpio(name:&str,data:&[u8],crc:bool)->Vec<u8>{
    let mut archive=Vec::new();
    cpio_record(&mut archive,name,data,0o100644,crc);
    cpio_record(&mut archive,"TRAILER!!!",&[],0,crc);
    archive
}
fn ar_member(out:&mut Vec<u8>,name:&str,data:&[u8]){
    let mut h=[b' ';60];
    assert!(name.len()<=16);
    h[..name.len()].copy_from_slice(name.as_bytes());
    let len=format!("{}",data.len());
    h[48..48+len.len()].copy_from_slice(len.as_bytes());
    h[58]=96;
    h[59]=b'\n';
    out.extend_from_slice(&h);
    out.extend_from_slice(data);
    if data.len()%2==1{out.push(b'\n')}
}
fn ar(members:&[(&str,&[u8])])->Vec<u8>{
    let mut out=b"!<arch>\n".to_vec();
    for (name,data) in members{ar_member(&mut out,name,data)}
    out
}
fn tar(path:&str,body:&[u8])->Vec<u8>{
    let mut h=[0u8;512];
    h[..path.len()].copy_from_slice(path.as_bytes());
    fn octal(out:&mut [u8],n:usize){
        let last=out.len()-1;
        let text=format!("{:0width$o}",n,width=last);
        out[..last].copy_from_slice(text.as_bytes());
        out[last]=0;
    }
    octal(&mut h[100..108],0o644);
    octal(&mut h[108..116],0);
    octal(&mut h[116..124],0);
    octal(&mut h[124..136],body.len());
    octal(&mut h[136..148],0);
    h[148..156].fill(b' ');
    h[156]=b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    let sum:usize=h.iter().map(|x|*x as usize).sum();
    octal(&mut h[148..156],sum);
    let mut result=h.to_vec();
    result.extend_from_slice(body);
    while result.len()%512!=0{result.push(0)}
    result.extend_from_slice(&[0u8;1024]);
    result
}
#[test]
fn newc_and_crc_roundtrip_and_no_overwrite(){
    let (root,dir)=fixture();
    fs::write(root.join("backup.cpio"),cpio("etc/notes.txt",b"safe contents",false)).unwrap();
    let manifest=preview_archive_tree(&dir,"backup.cpio").unwrap();
    assert_eq!(manifest.file_count,1);
    extract_archive_tree_new(&dir,"backup.cpio",&dir,"new-only",&manifest.sha256).unwrap();
    assert_eq!(fs::read(root.join("new-only/etc/notes.txt")).unwrap(),b"safe contents");
    assert!(observe_tree_completion(&dir,"new-only",&manifest).unwrap());
    assert_eq!(extract_archive_tree_new(&dir,"backup.cpio",&dir,"new-only",&manifest.sha256)
        .unwrap_err(),TreeExtractError::TargetExists);
    fs::write(root.join("crc.cpio"),cpio("usr/bin/tool",b"\x01\x02\x03",true)).unwrap();
    let members=cpio_tree_members(&dir,"crc.cpio").unwrap();
    assert_eq!(members[0].2,b"\x01\x02\x03");
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn cpio_gzip_and_crc_tamper_rejected(){
    let (root,dir)=fixture();
    let data=cpio("./usr/share/info.txt",b"compressed",true);
    let mut gz=GzEncoder::new(Vec::new(),Compression::default());
    gz.write_all(&data).unwrap();
    fs::write(root.join("backup.cpio.gz"),gz.finish().unwrap()).unwrap();
    let preview=preview_archive_tree(&dir,"backup.cpio.gz").unwrap();
    assert_eq!(preview.entries[0].path,"usr/share/info.txt");
    let mut corrupted=data;
    let p=corrupted.windows(10).position(|b|b==b"compressed").unwrap();
    corrupted[p]^=1;
    assert_eq!(parse_cpio_newc(&corrupted).unwrap_err(),CpioError::InvalidChecksum);
    assert_eq!(parse_cpio_newc(&cpio("../evil",b"x",false)).unwrap_err(),
        CpioError::UnsafeEntry);
    let mut links=cpio("foo",b"x",false);
    links[38..46].copy_from_slice(b"00000002");
    assert_eq!(parse_cpio_newc(&links).unwrap_err(),CpioError::UnsupportedEntry);
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn ar_gnu_long_names_and_debian_data_control_are_inert(){
    let (root,dir)=fixture();
    let long=b"long-object-file.o/\n";
    let a=ar(&[("//",long.as_slice()),("/0",b"ELF inert")]);
    fs::write(root.join("libtest.a"),&a).unwrap();
    assert_eq!(parse_ar(&a).unwrap()[0].name,"long-object-file.o");
    let members=ar_tree_members(&dir,"libtest.a").unwrap();
    assert_eq!(members[0].2,b"ELF inert");
    let control=tar("./control",b"Package: safe\nVersion: 1\n");
    let data=tar("./usr/share/app/hello.txt",b"Hello Debian");
    let mut gz=GzEncoder::new(Vec::new(),Compression::default());
    gz.write_all(&data).unwrap();
    let compressed_data=gz.finish().unwrap();
    let payload=ar(&[
        ("debian-binary/",b"2.0\n"),
        ("control.tar/",control.as_slice()),
        ("data.tar.gz/",compressed_data.as_slice()),
    ]);
    fs::write(root.join("safe.deb"),&payload).unwrap();
    let parsed=deb_virtual_members(&payload).unwrap();
    assert!(parsed.iter().any(|(p,_,b)|
        p=="control/control"&&b==b"Package: safe\nVersion: 1\n"));
    assert!(parsed.iter().any(|(p,_,b)|
        p=="data/usr/share/app/hello.txt"&&b==b"Hello Debian"));
    let manifest=preview_archive_tree(&dir,"safe.deb").unwrap();
    extract_archive_tree_new(&dir,"safe.deb",&dir,"deb-extracted",&manifest.sha256).unwrap();
    assert_eq!(fs::read(root.join("deb-extracted/data/usr/share/app/hello.txt"))
        .unwrap(),b"Hello Debian");
    assert!(!root.join("usr/share/app/hello.txt").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}
#[test]
fn malicious_or_truncated_ar_and_deb_are_refused(){
    let (root,dir)=fixture();
    let bad=ar(&[("../escape.o",b"bad")]);
    fs::write(root.join("unsafe.ar"),&bad).unwrap();
    assert_eq!(parse_ar(&bad).unwrap_err(),ArError::UnsafeName);
    assert!(preview_archive_tree(&dir,"unsafe.ar").is_err());
    let mut truncated=ar(&[("file.o/",b"trusted")]);
    truncated.pop();
    assert_eq!(parse_ar(&truncated).unwrap_err(),ArError::InvalidHeader);
    fs::write(root.join("bad.deb"),ar(&[("debian-binary/",b"3.0\n")])).unwrap();
    assert!(preview_archive_tree(&dir,"bad.deb").is_err());
    assert!(!root.join("escape.o").exists());
    drop(dir);fs::remove_dir_all(root).unwrap();
}

struct ApprovedHuman;
impl lu_core::conversation_authority::TrustedUserTurnVerifier for ApprovedHuman{
    fn verify_user_turn(&self,conversation:&str,user_turn:&str)->bool{
        conversation=="packages-chat"&&user_turn=="packages-user"
    }
}
struct ReadGrant;
impl lu_core::contracts::ToolAuthorizer for ReadGrant{
    fn authorize(&self,_:&lu_core::contracts::ToolInvocation,
        _:&lu_core::contracts::ToolDescriptor
    )->Result<(),lu_core::contracts::AuthorizationError>{Ok(())}
}
#[tokio::test]
async fn model_archive_list_uses_native_cpio_with_strict_output_budget(){
    use lu_core::{
        agent_tool_loop::{NativeToolTurn,ToolTurnOutcome},
        conversation_authority::{
            AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority,
        },
        contracts::ToolEffect,
        model::ModelToolCatalog,
        native_archive_inspect::register_archive_inspection_tool,
        registry::NativeToolRegistry,
    };
    let (root,dir)=fixture();
    let bytes=cpio("usr/local/share/file.txt",b"content",true);
    fs::write(root.join("archive.cpio"),&bytes).unwrap();
    let registry=NativeToolRegistry::new();
    let mut catalog=ModelToolCatalog::new();
    register_archive_inspection_tool(&registry,&mut catalog,&root).unwrap();
    let auth=ConversationAuthority::new();
    let origin=BoundCallOrigin{
        conversation_id:"packages-chat".into(),
        user_turn_id:"packages-user".into(),task_id:"package-read-task".into(),
    };
    auth.open_user_task(TaskAuthority{
        conversation_id:origin.conversation_id.clone(),
        user_turn_id:origin.user_turn_id.clone(),
        task_id:origin.task_id.clone(),execution_scope_id:"packages-scope".into(),
        capabilities:vec![AuthorizedCapability{
            tool_id:"lu.archive.list".into(),effect:ToolEffect::ReadWorkspace,
        }],expires_at:None,
    },&ApprovedHuman).unwrap();
    let available=std::collections::BTreeSet::from(["lu_archive_list".to_owned()]);
    let mut turn=NativeToolTurn::new_read_only(
        &catalog,&registry,&auth,origin,"packages-scope".into(),&available
    ).unwrap();
    let prompt=r#"{"kind":"tool","name":"lu_archive_list","arguments":{"path":"archive.cpio","max_entries":1}}"#;
    let reply=turn.handle_model_reply(prompt,&catalog,&registry,&auth,&ReadGrant)
        .await.unwrap();
    let ToolTurnOutcome::ToolResult{data,..}=reply else{panic!("No typed package result")};
    assert_eq!(data["ok"],true);
    assert_eq!(data["result"]["entries"][0]["path"],"usr/local/share/file.txt");
    assert_eq!(data["result"]["entries"].as_array().unwrap().len(),1);
    assert_eq!(data["result"]["total_entries"],1);
    assert_eq!(fs::read(root.join("archive.cpio")).unwrap(),bytes);
    assert_eq!(fs::read_dir(&root).unwrap().count(),1);
    // Windows cannot remove the fixture while registry-owned cap-std handles remain open.
    drop(turn);
    drop(catalog);
    drop(registry);
    drop(dir);
    fs::remove_dir_all(root).unwrap();
}
