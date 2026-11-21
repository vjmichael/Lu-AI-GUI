//! LU-IPC-01D-R1 original unchanged-source transport fixture.
//! Passing these tests is NOT installed OS token/ACL/package validation.
use lu_core::workspace_ipc::{proto, WIRE_MAJOR, WIRE_MINOR, MAX_FRAME_BYTES};
use lu_core::workspace_ipc_transport::{
    client_handshake, read_frame, server_handshake, write_frame,
    EnrolledPeer, PeerAdmission, PeerEvidence, ServerIdentity,
    TransportError,
};
use tokio::io::AsyncWriteExt;

const KEY:[u8;32]=[0x42;32];
const DIGEST:[u8;32]=[0x77;32];
fn hello()->proto::WorkspaceFrame {
    proto::WorkspaceFrame {
        protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
        request_id:"request-one".into(),
        session_ref:String::new(),sequence:0,
        body:Some(proto::workspace_frame::Body::Hello(proto::Hello {
            claimed_module_id:"lu.ide".into(),
            claimed_role:proto::ModuleRole::Workspace as i32,
            claimed_package_sha256:DIGEST.to_vec(),
            minimum_major:WIRE_MAJOR,maximum_major:WIRE_MAJOR,
            maximum_minor:WIRE_MINOR,
            requested_features:vec!["typed-calls".into()],
            client_nonce:vec![0x66;32],
        })),
    }
}
fn materials()->ServerIdentity {
    // Synthetic fixture: production MUST use OS CSPRNG and unique session IDs.
    ServerIdentity {
        server_nonce:[0x55;32],
        session_ref:"host-session-one".into(),
        core_epoch_ref:"core-epoch-one".into(),
    }
}
fn linux_peer()->PeerEvidence {
    PeerEvidence::Linux {uid:1000,gid:1000,pid:1234}
}
struct TestPolicy;
impl PeerAdmission for TestPolicy {
    fn admit(&self, peer:&PeerEvidence, hello:&proto::Hello)
        ->Result<EnrolledPeer,TransportError> {
        // Not a production verifier: native host MUST pin verified OS peer
        // process/package digest and retrieve a secure credential.
        match peer {
            PeerEvidence::Linux{uid:1000,pid:1234,..}
                if hello.claimed_module_id=="lu.ide"
                  && hello.claimed_package_sha256.as_slice()==DIGEST.as_slice()
                  =>Ok(EnrolledPeer{
                    module_id:"lu.ide".into(),
                    role:proto::ModuleRole::Workspace as i32,
                    package_sha256:DIGEST,handshake_key:KEY,
                  }),
            _=>Err(TransportError::PeerNotVerified),
        }
    }
}
fn frame(
    request:&str, session:&str, body:proto::workspace_frame::Body,
)->proto::WorkspaceFrame {
    proto::WorkspaceFrame {
        protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
        request_id:request.into(),session_ref:session.into(),sequence:0,
        body:Some(body),
    }
}
#[tokio::test]
async fn authenticated_duplex_roundtrip_and_session_binding(){
    let (mut server,mut client)=tokio::io::duplex(4096);
    let policy=TestPolicy;
    let server_work=server_handshake(
        &mut server,&linux_peer(),&policy,&materials());
    let client_work=client_handshake(&mut client,hello(),&KEY);
    let (received,opened)=tokio::join!(server_work,client_work);
    let host=received.unwrap();
    let remote=opened.unwrap();
    assert_eq!(host,remote);
    assert_eq!(host.module_id,"lu.ide");
    let valid=frame("health-1",&host.session_ref,
        proto::workspace_frame::Body::Health(proto::HealthRequest{}));
    write_frame(&mut client,&valid).await.unwrap();
    let on_server=read_frame(&mut server).await.unwrap();
    host.check_request(&on_server).unwrap();

    let other=frame("health-2","other-session",
        proto::workspace_frame::Body::Health(proto::HealthRequest{}));
    assert_eq!(host.check_request(&other),Err(TransportError::IdentityMismatch));
    assert!(host.check_request(&hello()).is_err()); // no second Hello
}
#[tokio::test]
async fn bad_client_mac_is_rejected_without_welcome(){
    let (mut server,mut client)=tokio::io::duplex(4096);
    let policy=TestPolicy;
    let server_work=async{
        let result=server_handshake(
            &mut server,&linux_peer(),&policy,&materials()).await;
        drop(server); // Close, never leave client waiting on a missing Welcome.
        result
    };
    let wrong_key=[0x11;32];
    let client_work=client_handshake(&mut client,hello(),&wrong_key);
    let (server_result,client_result)=tokio::join!(server_work,client_work);
    assert_eq!(server_result,Err(TransportError::MacMismatch));
    assert!(client_result.is_err());
}
#[tokio::test]
async fn forged_server_welcome_is_rejected(){
    let (mut fake_server,mut client)=tokio::io::duplex(4096);
    let attacker=async{
        let sent=read_frame(&mut fake_server).await.unwrap();
        let challenge=frame(&sent.request_id,"",
            proto::workspace_frame::Body::AuthChallenge(proto::AuthChallenge{
                server_nonce:vec![0x31;32],
            }));
        write_frame(&mut fake_server,&challenge).await.unwrap();
        let _proof=read_frame(&mut fake_server).await.unwrap();
        let forged=frame(&sent.request_id,"fake-host-session",
            proto::workspace_frame::Body::Welcome(proto::Welcome{
                selected_major:WIRE_MAJOR,selected_minor:WIRE_MINOR,
                core_epoch_ref:"fake-core-epoch".into(),
                enabled_features:vec![],server_hmac_sha256:vec![0;32],
            }));
        write_frame(&mut fake_server,&forged).await.unwrap();
    };
    let client_work=client_handshake(&mut client,hello(),&KEY);
    let (_,result)=tokio::join!(attacker,client_work);
    assert_eq!(result,Err(TransportError::MacMismatch));
}
#[tokio::test]
async fn unverified_windows_peer_never_enters_handshake(){
    let (mut server,_client)=tokio::io::duplex(128);
    let r=server_handshake(&mut server,&PeerEvidence::WindowsUnverified,
        &TestPolicy,&materials()).await;
    assert_eq!(r,Err(TransportError::PeerNotVerified));
}
#[tokio::test]
async fn huge_zero_and_truncated_prefix_fail_without_unbounded_allocation(){
    for n in [0,MAX_FRAME_BYTES+1] {
        let (mut rx,mut tx)=tokio::io::duplex(32);
        tx.write_all(&(n as u32).to_be_bytes()).await.unwrap();
        drop(tx);
        assert_eq!(read_frame(&mut rx).await,
            Err(TransportError::InvalidFrameLength));
    }
    let (mut rx,mut tx)=tokio::io::duplex(32);
    tx.write_all(&32u32.to_be_bytes()).await.unwrap();
    tx.write_all(&[0x08,0x01]).await.unwrap();
    drop(tx);
    assert_eq!(read_frame(&mut rx).await,
        Err(TransportError::Io(std::io::ErrorKind::UnexpectedEof)));
}
#[tokio::test]
async fn incompatible_version_denied_before_transport_roundtrip(){
    let (rx,mut tx)=tokio::io::duplex(128);
    let mut msg=hello();
    msg.protocol_major=99;
    assert!(write_frame(&mut tx,&msg).await.is_err());
    // The raw decoder is independently tested by workspace_ipc_v0.rs.
    drop(rx);
}
#[cfg(target_os="linux")]
#[tokio::test]
async fn linux_private_socket_observes_real_kernel_peer_and_rejects_unsafe_paths(){
    use lu_core::workspace_ipc_transport::linux;
    use std::os::unix::fs::{MetadataExt,PermissionsExt};
    let root=std::env::temp_dir().join(format!("lu-ipc-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).unwrap();
    let uid=std::fs::metadata(&root).unwrap().uid();
    let addr=root.join("core.sock");
    let listener=linux::bind_socket(&addr,uid).unwrap();
    assert_eq!(std::fs::metadata(&addr).unwrap().permissions().mode() & 0o777,0o600);
    assert!(linux::bind_socket(&addr,uid).is_err()); // cannot steal name
    let mut client=linux::connect(&addr).await.unwrap();
    let (mut server,evidence)=linux::accept_peer(&listener).await.unwrap();
    if let PeerEvidence::Linux{uid:actual_uid,pid,..}=evidence.clone(){
        assert_eq!(actual_uid,uid);
        assert_eq!(pid,std::process::id());
    }else{panic!("kernel peer missing");}
    // The native credential witness is passed to the explicit verifier.
    // This test deliberately does not claim package image verification.
    let server_roundtrip=async{
        let handshake=server_handshake(&mut server,&evidence,&LoopbackPolicy(uid),
            &materials()).await;
        handshake
    };
    let client_roundtrip=client_handshake(&mut client,hello(),&KEY);
    let (host,guest)=tokio::join!(server_roundtrip,client_roundtrip);
    assert!(host.is_ok());
    assert!(guest.is_ok());
    drop(server);drop(client);drop(listener);
    std::fs::remove_file(&addr).unwrap();
    // World-readable runtime directories fail before binding.
    std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(linux::bind_socket(&root.join("public.sock"),uid).is_err());
    std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).unwrap();
    let symlink=root.with_extension("link");
    std::os::unix::fs::symlink(&root,&symlink).unwrap();
    assert!(linux::bind_socket(&symlink.join("stolen.sock"),uid).is_err());
    std::fs::remove_file(&symlink).unwrap();
    std::fs::remove_dir(&root).unwrap();
}
#[cfg(target_os="linux")]
struct LoopbackPolicy(u32);
#[cfg(target_os="linux")]
impl PeerAdmission for LoopbackPolicy {
    fn admit(&self,peer:&PeerEvidence,hello:&proto::Hello)
        ->Result<EnrolledPeer,TransportError>{
        match peer{
            PeerEvidence::Linux{uid,pid,..}
                if *uid==self.0 && *pid==std::process::id()
                && hello.claimed_module_id=="lu.ide"
                && hello.claimed_package_sha256.as_slice()==DIGEST.as_slice()
                =>Ok(EnrolledPeer{
                    module_id:"lu.ide".into(),
                    role:proto::ModuleRole::Workspace as i32,
                    package_sha256:DIGEST,handshake_key:KEY,
                }),
            _=>Err(TransportError::PeerNotVerified),
        }
    }
}
#[cfg(windows)]
#[tokio::test]
async fn windows_pipe_is_local_single_instance_and_unauthorized_until_os_attested(){
    use lu_core::workspace_ipc_transport::windows;
    let pipe=format!(r"\\.\pipe\lu-ipc-{:032x}",std::process::id());
    let mut server=windows::bind_one(&pipe).unwrap();
    assert!(windows::bind_one(&pipe).is_err()); // first-instance fence.
    let mut client=windows::connect(&pipe).unwrap();
    let evidence=windows::accept_peer(&server).await.unwrap();
    let pinned=match &evidence{
        PeerEvidence::WindowsObserved{pid,session_id,user_sid,image_path}=>{
            assert_eq!(*pid,std::process::id());
            assert!(!user_sid.is_empty()&&!image_path.is_empty());
            lu_core::workspace_ipc_transport::WindowsPinnedEnrollment {
                expected_pid:*pid,expected_session_id:*session_id,
                expected_user_sid:user_sid.clone(),
                expected_executable_path:image_path.clone(),
                expected_module_id:"lu.ide".into(),
                expected_role:proto::ModuleRole::Workspace as i32,
                expected_package_sha256:DIGEST,handshake_key:KEY,
            }
        }
        _=>panic!("missing Win32 OS witness"),
    };
    // Native process/session/SID observed. The host still must independently
    // attest the executable's enrolled, signer-verified package.
    let server_work=server_handshake(&mut server,&evidence,&pinned,&materials());
    let client_work=client_handshake(&mut client,hello(),&KEY);
    let (host,guest)=tokio::join!(server_work,client_work);
    assert_eq!(host.unwrap(),guest.unwrap());

    // Verified SID is not sufficient for forged PID or executable identity.
    let forged_pid=lu_core::workspace_ipc_transport::WindowsPinnedEnrollment {
        expected_pid:std::process::id().wrapping_add(1),..pinned
    };
    let h=hello();
    let Some(proto::workspace_frame::Body::Hello(claimed))=h.body.as_ref() else{unreachable!()};
    assert!(forged_pid.admit(&evidence,claimed).is_err());
}
