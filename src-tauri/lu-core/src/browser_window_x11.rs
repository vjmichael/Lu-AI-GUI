//! TOOLS-10B X11 visibility for an owned headed Chromium child.
//! The trusted host supplies a pinned xdotool executable and Child handle;
//! there is no model-supplied PID, title, shell command or arbitrary HWND.
//! Wayland is unsupported here and must fail explicitly, not pretend hidden.
use std::{io,path::Path,process::{Child,Command}};
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
pub fn set_owned_browser_visible(
    child:&Child,
    xdotool:&Path,
    visible:bool,
)->Result<(),X11VisibilityError>{
    approved_display()?;
    if !xdotool.is_absolute() || !xdotool.is_file() {
        return Err(X11VisibilityError::ToolUnavailable);
    }
    let pid=child.id();
    if pid==0{return Err(X11VisibilityError::WindowNotFound);}
    // Always search mapped AND unmapped windows of the exact owned PID.
    // An unmapped window must remain discoverable when human verification
    // requires re-showing the original Chromium session.
    let output=Command::new(xdotool).args(["search","--pid",&pid.to_string()])
        .output().map_err(|_|X11VisibilityError::ToolUnavailable)?;
    if !output.status.success(){return Err(X11VisibilityError::WindowNotFound);}
    let ids=candidate_window_ids(&output.stdout)?;
    for id in ids{
        let operation=if visible {"windowmap"} else {"windowunmap"};
        let status=Command::new(xdotool).args([operation,"--sync",&id])
            .status().map_err(|_|X11VisibilityError::WindowOperationFailed)?;
        if !status.success(){return Err(X11VisibilityError::WindowOperationFailed);}
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn untrusted_window_names_and_non_numeric_ids_are_rejected(){
        assert_eq!(candidate_window_ids(b"Chromium\n"),Err(X11VisibilityError::WindowNotFound));
        assert_eq!(candidate_window_ids(b"99\n100\n").unwrap(),vec!["99","100"]);
        assert_eq!(candidate_window_ids(b""),Err(X11VisibilityError::WindowNotFound));
    }
}
