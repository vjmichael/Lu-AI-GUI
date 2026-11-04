//! Audited Windows-only browser visibility boundary.
//! Only a native Lu host holding the Child may use this interface: never
//! accept arbitrary target PIDs or window titles from model/browser content.
//! The browser is launched headed; Win32 hides/reveals its existing top-level
//! window without restarting Chromium or discarding the browser profile.
#![cfg(windows)]
use std::{io, process::Child};
type Hwnd = isize;
type Lparam = isize;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(Hwnd,Lparam)->i32, param:Lparam)->i32;
    fn GetWindowThreadProcessId(hwnd:Hwnd,process_id:*mut u32)->u32;
    fn IsWindow(hwnd:Hwnd)->i32;
    fn IsWindowVisible(hwnd:Hwnd)->i32;
    fn ShowWindow(hwnd:Hwnd,command:i32)->i32;
}
struct Search { pid:u32, hwnd:Option<Hwnd> }
unsafe extern "system" fn match_process(hwnd:Hwnd,data:Lparam)->i32 {
    // SAFETY: EnumWindows invokes synchronously while Search remains alive.
    let search=unsafe {&mut *(data as *mut Search)};
    let mut pid=0u32;
    // SAFETY: hwnd is a system-provided handle, pointer is valid.
    unsafe {GetWindowThreadProcessId(hwnd,&mut pid)};
    if pid==search.pid && unsafe {IsWindow(hwnd)}!=0 {
        search.hwnd=Some(hwnd);
        return 0;
    }
    1
}
fn window_for_owned_child(child:&Child)->io::Result<Hwnd> {
    let pid=child.id();
    if pid==0 {return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid browser child"));}
    let mut search=Search {pid,hwnd:None};
    // EnumWindows returns 0 both for callback early stop and failure; an
    // observed owned HWND is the authority for success.
    unsafe {EnumWindows(match_process,&mut search as *mut _ as Lparam)};
    search.hwnd.ok_or_else(||io::Error::new(
        io::ErrorKind::NotFound,"no top-level window belongs to this Lu-owned child"
    ))
}
pub fn set_owned_browser_visible(child:&Child, visible:bool)->io::Result<()> {
    let hwnd=window_for_owned_child(child)?;
    // SAFETY: window was matched to the host-owned process, not user input.
    unsafe {ShowWindow(hwnd,if visible {SW_SHOW} else {SW_HIDE})};
    // ShowWindow's return value indicates *previous* visibility, not success.
    let now=unsafe {IsWindowVisible(hwnd)}!=0;
    if now!=visible {
        return Err(io::Error::other("native browser window visibility did not change"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_fall_back_to_an_arbitrary_window() {
        // This module deliberately exposes no window-by-title or
        // process-ID-from-model API. A valid Child is required.
        let _api:set_owned_browser_visible_type = set_owned_browser_visible;
    }
    #[allow(non_camel_case_types)]
    type set_owned_browser_visible_type=fn(&Child,bool)->io::Result<()>;
}
