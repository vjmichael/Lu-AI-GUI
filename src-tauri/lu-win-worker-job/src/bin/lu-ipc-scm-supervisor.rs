//! Fixed-name SCM service entry point for Lu Core's protected IPC
//! supervisor. This process never auto-enrolls an IDE, accepts a client path,
//! starts a tool worker, or emits an executor termination receipt. Those
//! operations require a separately signed/approved installer and protected
//! service-owned worker control plane. RUNNING means only an SCM-hosted
//! LocalSystem service is alive; it is NOT proof of effect recovery.
#[cfg(windows)]
mod native {
    use std::{ffi::c_void,io,ptr,sync::atomic::{AtomicBool,Ordering}};
    use lu_win_worker_job::{
        ipc_credential_broker::require_system_service_context,
        ipc_scm_owned_jobs::{ScmJobKeeper,SupervisedJobLane},
        ipc_scm_control::serve_control,
        ipc_scm_receipts::{
            persist_executor_stop,installed_public_key_for_protected_installer,
        },
        ipc_installer_enrollment::{
            read_installed_scm_receipt_public_key,protected_executor_receipts_root,
        },
    };

    type Handle=*mut c_void;
    const SERVICE_WIN32_OWN_PROCESS:u32=0x10;
    const SERVICE_STOPPED:u32=1;
    const SERVICE_START_PENDING:u32=2;
    const SERVICE_STOP_PENDING:u32=3;
    const SERVICE_RUNNING:u32=4;
    const SERVICE_ACCEPT_STOP:u32=1;
    const SERVICE_ACCEPT_SHUTDOWN:u32=4;
    const SERVICE_CONTROL_STOP:u32=1;
    const SERVICE_CONTROL_INTERROGATE:u32=4;
    const SERVICE_CONTROL_SHUTDOWN:u32=5;
    const ERROR_CALL_NOT_IMPLEMENTED:u32=120;
    const ERROR_SERVICE_SPECIFIC_ERROR:u32=1066;
    const SERVICE_NAME:&str="LuCoreIpcSupervisor";
    static STOP:AtomicBool=AtomicBool::new(false);

    #[repr(C)]
    #[derive(Default)]
    struct ServiceStatus{
        service_type:u32,
        current_state:u32,
        controls_accepted:u32,
        win32_exit_code:u32,
        service_specific_exit_code:u32,
        check_point:u32,
        wait_hint:u32,
    }
    const _:()={assert!(std::mem::size_of::<ServiceStatus>()==28);};
    #[repr(C)]
    struct ServiceTableEntryW{
        name:*mut u16,
        service_main:Option<unsafe extern "system" fn(u32,*mut *mut u16)>,
    }
    #[link(name="advapi32")]
    unsafe extern "system"{
        fn StartServiceCtrlDispatcherW(table:*const ServiceTableEntryW)->i32;
        fn RegisterServiceCtrlHandlerExW(
            name:*const u16,
            handler:Option<unsafe extern "system" fn(u32,u32,*mut c_void,*mut c_void)->u32>,
            context:*mut c_void,
        )->Handle;
        fn SetServiceStatus(handle:Handle,status:*const ServiceStatus)->i32;
    }
    fn report(handle:Handle,state:u32,checkpoint:u32)->io::Result<()>{
        let record=ServiceStatus{
            service_type:SERVICE_WIN32_OWN_PROCESS,
            current_state:state,
            controls_accepted:if state==SERVICE_RUNNING{
                SERVICE_ACCEPT_STOP|SERVICE_ACCEPT_SHUTDOWN
            }else{0},
            win32_exit_code:if state==SERVICE_STOPPED && checkpoint!=0{
                ERROR_SERVICE_SPECIFIC_ERROR
            }else{0},
            service_specific_exit_code:if state==SERVICE_STOPPED{checkpoint}else{0},
            check_point:if state==SERVICE_START_PENDING ||
                state==SERVICE_STOP_PENDING{1}else{0},
            wait_hint:if state==SERVICE_START_PENDING ||
                state==SERVICE_STOP_PENDING{5000}else{0},
        };
        if unsafe{SetServiceStatus(handle,&record)}==0{
            Err(io::Error::last_os_error())
        }else{Ok(())}
    }
    unsafe extern "system" fn control(
        code:u32,_event_type:u32,_event:*mut c_void,_context:*mut c_void,
    )->u32{
        match code{
            SERVICE_CONTROL_STOP|SERVICE_CONTROL_SHUTDOWN=>{
                STOP.store(true,Ordering::Release);
                0
            }
            SERVICE_CONTROL_INTERROGATE=>0,
            _=>ERROR_CALL_NOT_IMPLEMENTED,
        }
    }
    unsafe extern "system" fn service_main(
        _argc:u32,_argv:*mut *mut u16,
    ){
        let name:Vec<u16>=SERVICE_NAME.encode_utf16()
            .chain(std::iter::once(0)).collect();
        let handle=unsafe{
            RegisterServiceCtrlHandlerExW(name.as_ptr(),Some(control),ptr::null_mut())
        };
        if handle.is_null(){return;}
        if report(handle,SERVICE_START_PENDING,0).is_err(){return;}
        // Reject direct elevation or a thread impersonating another user.
        // Only SCM running as LocalSystem qualifies, not a signed executable
        // launched manually by an interactive administrator.
        if require_system_service_context().is_err(){
            let _=report(handle,SERVICE_STOPPED,1);
            return;
        }
        // The keeper lives entirely in this SCM process. The Core/UI is
        // never handed the owning Job handles. Admission of real workers
        // remains disabled until an installer-pinned, authenticated service
        // control plane can supply already-restricted suspended children.
        let keeper=match ScmJobKeeper::new_for_service(){
            Ok(value)=>value,
            Err(_)=>{
                let _=report(handle,SERVICE_STOPPED,2);
                return;
            }
        };
        // Installer-owned public-key pin must match the OS-held private key.
        // A plain SYSTEM command shell cannot create/rotate one silently.
        let approved=read_installed_scm_receipt_public_key();
        let actual=installed_public_key_for_protected_installer();
        if !matches!((approved,actual),(Ok(a),Ok(b)) if a==b)
           ||protected_executor_receipts_root().is_err(){
            let _=report(handle,SERVICE_STOPPED,4);
            return;
        }
        // The only private control listener. A stopped/crashed listener
        // prevents new admissions; the service still reaps every owned Job.
        let control_ok=serve_control(&keeper,&STOP,||
            report(handle,SERVICE_RUNNING,0)).is_ok();
        STOP.store(true,Ordering::Release);
        let _=report(handle,SERVICE_STOP_PENDING,0);
        let stopped=match keeper.stop_all(){
            Ok(witnesses)=>witnesses.iter().all(|witness|
                witness.lane()!=SupervisedJobLane::CoreExecutor
                    ||persist_executor_stop(witness).is_ok()
            ),
            Err(_)=>false,
        };
        let _=report(handle,SERVICE_STOPPED,if stopped&&control_ok{0}else{3});
    }
    pub fn run()->io::Result<()>{
        // SCM calls service_main only through this registered table; a plain
        // user-shell invocation fails StartServiceCtrlDispatcherW.
        let mut name:Vec<u16>=SERVICE_NAME.encode_utf16()
            .chain(std::iter::once(0)).collect();
        let table=[
            ServiceTableEntryW{name:name.as_mut_ptr(),service_main:Some(service_main)},
            ServiceTableEntryW{name:ptr::null_mut(),service_main:None},
        ];
        if unsafe{StartServiceCtrlDispatcherW(table.as_ptr())}==0{
            Err(io::Error::last_os_error())
        }else{Ok(())}
    }
}
#[cfg(windows)]
fn main(){
    if native::run().is_err(){
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main(){
    eprintln!("Lu IPC SCM supervisor is Windows-only");
    std::process::exit(1);
}
