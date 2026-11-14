//! 6E-05B native physical OS vault acceptance diagnostic, NOT the IDE app.
//! Explicit local operator invocation only. No token argument, environment
//! token, credential setter, remote write, browser route or native OS grant.
//! Six bounded API GETs are fixed inside github_remote_live_policy.rs.
//! CI cargo CHECK compiles the actual native OS vault adapter without
//! accessing credentials or contacting any remote repository.
#[cfg(all(feature="fin03-native-vault",any(windows,target_os="linux")))]
fn main(){
    use lu_ide_workspace::{
        github_remote_live_policy::NativeOsGitHubReadVault,
        IdeWorkspaceRegistry,
    };
    use std::{env,fs,path::PathBuf,process};
    let args=env::args_os().collect::<Vec<_>>();
    if args.len()!=4
        ||args[3]!="--confirm-os-keychain-read"
    {
        eprintln!("FIN03_6E05B_DIAG=DENIED_EXPLICIT_LOCAL_OPERATOR_INVOCATION_REQUIRED");
        process::exit(2);
    }
    let catalog=PathBuf::from(&args[1]);
    let project=match args[2].to_str(){
        Some(p) if p.starts_with("ide-")&&p.len()==36
            &&p[4..].bytes().all(|b|b.is_ascii_hexdigit()) => p,
        _=>{
            eprintln!("FIN03_6E05B_DIAG=DENIED_REGISTERED_PROJECT_ID_INVALID");
            process::exit(2);
        },
    };
    let meta=fs::symlink_metadata(&catalog).ok();
    if !catalog.is_absolute()
        ||meta.as_ref().is_none_or(|m|!m.is_file()||m.file_type().is_symlink())
    {
        eprintln!("FIN03_6E05B_DIAG=DENIED_CATALOG_NOT_PLAIN_ABSOLUTE_FILE");
        process::exit(2);
    }
    let reg=match IdeWorkspaceRegistry::open(catalog){
        Ok(v)=>v,
        Err(_)=>{
            eprintln!("FIN03_6E05B_DIAG=DENIED_INVALID_SAVED_NATIVE_PROJECT_CATALOG");
            process::exit(3);
        },
    };
    if reg.reopen(project).is_err(){
        eprintln!("FIN03_6E05B_DIAG=DENIED_PROJECT_NOT_ALREADY_REGISTERED");
        process::exit(3);
    }
    let vault=NativeOsGitHubReadVault::default();
    let result=reg.inspect_live_github_branch_policy(project,&vault);
    match result{
        Ok(r) if r.authenticated_host_read&&r.source_rechecked
            &&r.remote_ref_same_on_recheck&&r.policy_same_on_recheck
            &&r.branch_protection_observed&&r.branch_rules_observed
            &&r.error_code.is_none() && !r.live_ref_compare_and_swap_authorized
            && !r.remote_write_route_available && !r.remote_mutation_performed
            && !r.native_approval_verified =>
        {
            println!("FIN03_6E05B_NATIVE_AUTHENTICATED_SIX_GET=PASS");
            println!("FIN03_6E05B_BRANCH_REF_AND_EVALUATED_RULES=STABLE_AT_OBSERVATION");
            println!("FIN03_6E05B_REMOTE_PUBLICATION=NEVER_AUTHORIZED");
        }
        Ok(_)=>{
            eprintln!("FIN03_6E05B_DIAG=DENIED_INCOMPLETE_OR_UNVERIFIED_READBACK");
            process::exit(4);
        }
        Err(code)=>{
            // Code is a static first-party classifier, not request body.
            eprintln!("FIN03_6E05B_DIAG={code}");
            process::exit(4);
        }
    }
}
#[cfg(not(all(feature="fin03-native-vault",any(windows,target_os="linux"))))]
fn main(){
    eprintln!("FIN03_6E05B_DIAG=DISABLED_NATIVE_OS_VAULT_FEATURE_REQUIRED");
    std::process::exit(2);
}
