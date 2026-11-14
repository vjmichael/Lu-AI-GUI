//! Independently restricted, installed SCM-only target Job steward.
//! Default OFF. Does not launch a debuggee, debugger or generic command.
#[cfg(all(windows,debug_assertions,feature="fin04w2-split-native"))]
fn main(){
    if std::env::args_os().nth(1).is_some(){
        eprintln!("DENIED: target SCM service accepts no arguments");
        std::process::exit(2);
    }
    if lu_ide_execution_supervisor::windows_fin04w2_target_service::dispatch().is_err(){
        eprintln!("DENIED: target SCM service requires installed independent SCM custody");
        std::process::exit(2);
    }
}
#[cfg(not(all(windows,debug_assertions,feature="fin04w2-split-native")))]
fn main(){
    eprintln!("DENIED: FIN-04W2 split target service disabled");
    std::process::exit(2);
}
