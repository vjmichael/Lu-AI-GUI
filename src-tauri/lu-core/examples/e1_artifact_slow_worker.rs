//! Real native slow artifact worker for cancellation/timeout acceptance.
//! It does not have a path/CLI interface beyond the two fixed host args.
use std::ffi::OsStr;
fn main(){
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(OsStr::new("--lu-e1-artifact-one-shot"))
      ||!matches!(args.next().as_deref(),
        Some(x) if x==OsStr::new("docx")||x==OsStr::new("xlsx"))
      ||args.next().is_some(){std::process::exit(2)}
    std::thread::sleep(std::time::Duration::from_secs(22));
}
