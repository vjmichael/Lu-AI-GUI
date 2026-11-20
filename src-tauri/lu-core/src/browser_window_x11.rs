//! TOOLS-10B X11 visibility for an owned headed Chromium child.
//! The trusted host supplies a pinned xdotool executable and Child handle;
//! there is no model-supplied PID, title, shell command or arbitrary HWND.
//! Wayland is unsupported here and must fail explicitly, not pretend hidden.
use std::{io::Read,path::Path,process::{Child,Command,Stdio},
    sync::mpsc,thread,time::{Duration,Instant}};
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum X11VisibilityError {
    UnsupportedDisplay,
    ToolUnavailable,
    WindowNotFound,
    WindowOperationFailed,
}
fn approved_display()->Result<(),X11VisibilityError>{
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_none() {
        return Err(X11VisibilityError::UnsupportedDisplay);
    }
    if std::env::var_os("DISPLAY").is_none() {
        return Err(X11VisibilityError::UnsupportedDisplay);
    }
    Ok(())
}
fn candidate_window_ids(output:&[u8])->Result<Vec<String>,X11VisibilityError>{
    let text=std::str::from_utf8(output).map_err(|_|X11VisibilityError::WindowNotFound)?;
    let mut ids=Vec::new();
    for line in text.lines(){
        if line.is_empty() {continue;}
        if line.len()>20 || !line.bytes().all(|c|c.is_ascii_digit()) {
            return Err(X11VisibilityError::WindowNotFound);
        }
        ids.push(line.to_owned());
        if ids.len()>16 {return Err(X11VisibilityError::WindowNotFound);}
    }
    if ids.is_empty(){return Err(X11VisibilityError::WindowNotFound);}
    Ok(ids)
}
// A stalled X server must not extend the original command deadline. Only
// the fixed, directly spawned helper is terminated; the original browser
// subtree still requires its independent OS-owner cleanup receipt.
fn bounded_x11_helper(mut command:Command,deadline:Instant)
    ->Result<Vec<u8>,X11VisibilityError>{
    if Instant::now()>=deadline{return Err(X11VisibilityError::WindowOperationFailed);}
    let mut child=command.stdin(Stdio::null()).stdout(Stdio::piped())
        .stderr(Stdio::null()).spawn().map_err(|_|X11VisibilityError::ToolUnavailable)?;
    let stdout=child.stdout.take().ok_or(X11VisibilityError::WindowOperationFailed)?;
    let (tx,rx)=mpsc::sync_channel(1);
    thread::spawn(move||{
        let mut bytes=Vec::new();
        let result=stdout.take(513).read_to_end(&mut bytes).map(|_|bytes);
        let _=tx.send(result);
    });
    let result=(||{
        loop{
            if Instant::now()>=deadline{return Err(X11VisibilityError::WindowOperationFailed);}
            if let Some(status)=child.try_wait().map_err(|_|X11VisibilityError::WindowOperationFailed)?{
                if !status.success(){return Err(X11VisibilityError::WindowOperationFailed);}
                break;
            }
            thread::sleep(Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())));
        }
        let bytes=rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_|X11VisibilityError::WindowOperationFailed)?
            .map_err(|_|X11VisibilityError::WindowOperationFailed)?;
        if bytes.len()>512||Instant::now()>=deadline{
            return Err(X11VisibilityError::WindowOperationFailed);
        }
        Ok(bytes)
    })();
    if result.is_err() && !matches!(child.try_wait(),Ok(Some(_))){
        let _=child.kill();
        thread::spawn(move||{let _=child.wait();});
    }
    result
}
pub fn set_owned_browser_visible(
    child:&Child,
    xdotool:&Path,
    visible:bool,
)->Result<(),X11VisibilityError>{
    set_verified_browser_pid_visible(child.id(),xdotool,visible,
        Instant::now()+Duration::from_secs(30))
}
/// The PID is taken only from the original SEC-06C manager's verified
/// Chromium cgroup admission; it is NEVER a tool/model/UI-selected PID.
/// The caller checks the same exact live kernel leaf before each transition.
pub(crate) fn set_verified_browser_pid_visible(
    pid:u32,xdotool:&Path,visible:bool,deadline:Instant,
)->Result<(),X11VisibilityError>{
    approved_display()?;
    if !xdotool.is_absolute() || !xdotool.is_file() {
        return Err(X11VisibilityError::ToolUnavailable);
    }
    if pid<=1{return Err(X11VisibilityError::WindowNotFound);}
    // Always search mapped AND unmapped windows of the exact owned PID.
    // An unmapped window must remain discoverable when human verification
    // requires re-showing the original Chromium session.
    let mut search=Command::new(xdotool);
    search.args(["search","--pid",&pid.to_string()]);
    let output=bounded_x11_helper(search,deadline)?;
    let ids=candidate_window_ids(&output)?;
    for id in ids{
        let operation=if visible {"windowmap"} else {"windowunmap"};
        let mut action=Command::new(xdotool);
        action.args([operation,"--sync",&id]);
        bounded_x11_helper(action,deadline)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn helper_deadline_bounds_a_stalled_process(){
        let mut command=Command::new("/bin/sleep");command.arg("30");
        let start=Instant::now();
        assert_eq!(bounded_x11_helper(command,start+Duration::from_millis(150)),
            Err(X11VisibilityError::WindowOperationFailed));
        assert!(start.elapsed()<Duration::from_secs(2));
    }
    #[test]
    fn helper_rejects_unbounded_output_and_nonzero_exit(){
        let mut command=Command::new("/bin/sh");command.args(["-c","printf '%0513d' 0"]);
        assert_eq!(bounded_x11_helper(command,Instant::now()+Duration::from_secs(2)),
            Err(X11VisibilityError::WindowOperationFailed));
        let mut command=Command::new("/bin/sh");command.args(["-c","exit 1"]);
        assert_eq!(bounded_x11_helper(command,Instant::now()+Duration::from_secs(2)),
            Err(X11VisibilityError::WindowOperationFailed));
        let mut command=Command::new("/bin/sh");command.args(["-c","printf '42\\n'"]);
        assert_eq!(bounded_x11_helper(command,Instant::now()+Duration::from_secs(2)).unwrap(),b"42\n");
    }
    #[test]
    fn untrusted_window_names_and_non_numeric_ids_are_rejected(){
        assert_eq!(candidate_window_ids(b"Chromium\n"),Err(X11VisibilityError::WindowNotFound));
        assert_eq!(candidate_window_ids(b"99\n100\n").unwrap(),vec!["99","100"]);
        assert_eq!(candidate_window_ids(b""),Err(X11VisibilityError::WindowNotFound));
    }
}
