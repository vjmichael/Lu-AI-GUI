//! OBSERVE-WIN-CAPTURE-05: one-shot Windows.Graphics.Capture picker and
//! in-memory D3D11 frame readback, launched ONLY through Lu's existing
//! BackgroundProcessSupervisor after separate native user confirmation.
//!
//! Does not save screenshots, stream pixels to Lu/LLM, capture microphone,
//! persist window titles, enumerate arbitrary windows, or grant UI control.
//! The OS picker chooses exactly one window OR display. Current OS-pick
//! source type is not independently attested as a window identity; do not
//! bind its output to Lu's app-only observation scope or infer test PASS.
//! Task stdout follows the existing bounded P / D host receipt protocol.
#![deny(unsafe_code)]

const MAX_FRAME_WIDTH:u32=7680;
const MAX_FRAME_HEIGHT:u32=4320;
const MAX_FRAME_BYTES:usize=(MAX_FRAME_WIDTH as usize)*(MAX_FRAME_HEIGHT as usize)*4;
/// Validates the actual mapped D3D11 row layout. Accepts ordinary pitch
/// padding only within a fixed bounded allocation; rejects truncated frames.
fn frame_geometry_valid(width:u32,height:u32,row_pitch:u32,buffer_len:usize)->bool {
    if width==0||height==0||width>MAX_FRAME_WIDTH||height>MAX_FRAME_HEIGHT{return false;}
    let Some(row_bytes)=(width as usize).checked_mul(4) else{return false;};
    let Some(pitch)=usize::try_from(row_pitch).ok() else{return false;};
    if pitch<row_bytes||pitch>row_bytes.saturating_add(4096){return false;}
    let Some(mapped_len)=pitch.checked_mul(height as usize) else{return false;};
    let Some(useful_end)=pitch.checked_mul(height as usize-1)
        .and_then(|n|n.checked_add(row_bytes)) else{return false;};
    let max_padded=MAX_FRAME_BYTES+(MAX_FRAME_HEIGHT as usize)*4096;
    mapped_len<=max_padded&&buffer_len>=useful_end&&buffer_len<=max_padded
}
// P06 privacy primitive for a FUTURE OS-attested sensitive-region mask.
// Geometry-only rectangles supplied by the renderer/model are NOT trusted
// evidence. There is deliberately no live caller that emits unmasked pixels.
// Actual WGC receipt keeps suppressing the full frame until native identity,
// trusted region detection and operator acceptance are independently proven.
#[derive(Clone,Copy,Debug)]
#[allow(dead_code)]
struct SensitiveRect{left:u32,top:u32,width:u32,height:u32}
#[allow(dead_code)]
fn mask_sensitive_bgra_regions(
    frame_width:u32,frame_height:u32,row_pitch:u32,
    frame:&mut [u8],regions:&[SensitiveRect]
)->bool {
    const MAX_MASKS:usize=4096;
    // Empty/incomplete evidence cannot be interpreted as "nothing private".
    // All rectangles are validated BEFORE any byte is changed so one bad
    // rectangle never leaves a partially redacted buffer available to callers.
    if regions.is_empty()||regions.len()>MAX_MASKS
        ||!frame_geometry_valid(frame_width,frame_height,row_pitch,frame.len()){
        return false;
    }
    for rect in regions{
        if rect.width==0||rect.height==0
            ||rect.left.checked_add(rect.width).is_none_or(|v|v>frame_width)
            ||rect.top.checked_add(rect.height).is_none_or(|v|v>frame_height){
            return false;
        }
    }
    let pitch=row_pitch as usize;
    for rect in regions{
        let left=(rect.left as usize)*4;
        let right=((rect.left+rect.width) as usize)*4;
        for y in rect.top..rect.top+rect.height{
            let base=y as usize*pitch;
            frame[base+left..base+right].fill(0);
        }
    }
    true
}

/// Zero-by-default extraction for a FUTURE separately consented,
/// source-attested selected-region preview. The caller must obtain an
/// independent native user review and exact WGC source/coordinate binding
/// before authorizing ANY region. This primitive itself grants nothing;
/// it is intentionally NOT called from the active WGC capture path.
#[derive(Clone,Copy,Debug)]
#[allow(dead_code)]
struct ApprovedBgraRect{left:u32,top:u32,width:u32,height:u32}
#[allow(dead_code)]
fn approved_regions_only_bgra(
    width:u32,height:u32,row_pitch:u32,src:&[u8],
    approved:&[ApprovedBgraRect]
)->Option<Vec<u8>>{
    const MAX_APPROVED:usize=64;
    if approved.is_empty()||approved.len()>MAX_APPROVED
        ||!frame_geometry_valid(width,height,row_pitch,src.len()){
        return None;
    }
    // Validate *every* region before touching raw pixels or allocating an
    // output frame. Reject overflow and any inconsistent source geometry.
    for rect in approved {
        if rect.width==0||rect.height==0
            ||rect.left.checked_add(rect.width).is_none_or(|v|v>width)
            ||rect.top.checked_add(rect.height).is_none_or(|v|v>height){
            return None;
        }
    }
    let pitch=row_pitch as usize;
    let size=pitch.checked_mul(height as usize)?;
    let mut sanitized=vec![0u8;size];
    // No implicit full-frame copy. BGRA outside the reviewed regions and
    // every byte of D3D11 row-pitch padding stay zero, even when source
    // padding contains driver bytes.
    for rect in approved {
        let left=rect.left as usize*4;
        let right=(rect.left+rect.width) as usize*4;
        for y in rect.top..rect.top+rect.height{
            let offset=y as usize*pitch;
            sanitized[offset+left..offset+right]
                .copy_from_slice(&src[offset+left..offset+right]);
        }
    }
    Some(sanitized)
}

/// Fail-closed FULL-FRAME redaction: no raw or selectively visible pixels
/// contribute to the host receipt. The OS readback only establishes a
/// geometrically valid WGC callback. No trusted sensitive-region classifier
/// exists yet, so image evidence/pixel previews are categorically forbidden.
/// The digest commits to nonce, geometry and a wholly blank output frame.
/// Not suitable for visual comparison or as a screenshot fingerprint.
fn frame_receipt(nonce:&str,width:u32,height:u32,row_pitch:u32,pixels:&[u8])->Option<String> {
    use sha2::{Digest,Sha256};
    if nonce.len()!=32||!nonce.bytes().all(|b|b.is_ascii_hexdigit())
        ||!frame_geometry_valid(width,height,row_pitch,pixels.len()) {return None;}
    let pitch=row_pitch as usize;
    let active_bytes=(width as usize)*4;
    let mut hash=Sha256::new();
    hash.update(b"Lu-WGC-whole-frame-redacted-v2");
    hash.update(nonce.as_bytes());
    hash.update(width.to_le_bytes());
    hash.update(height.to_le_bytes());
    let _=pitch;
    let _=pixels;
    const BLANK:[u8;4096]=[0;4096];
    for _ in 0..height as usize {
        let mut remaining=active_bytes;
        while remaining>0{
            let n=remaining.min(BLANK.len());
            hash.update(&BLANK[..n]);
            remaining-=n;
        }
    }
    Some(format!("{:x}",hash.finalize()))
}

#[cfg(test)]
mod frame_geometry_tests {
    use super::*;
    #[test]
    fn accepts_mapped_bgra_frame_with_bounded_pitch_padding(){
        assert!(frame_geometry_valid(1920,1080,7680,7680*1080));
        assert!(frame_geometry_valid(1919,1080,7680,7680*1080));
    }
    #[test]
    fn whole_frame_redacted_receipt_ignores_all_original_pixel_content(){
        let nonce="00112233445566778899aabbccddeeff";
        let mut padded=vec![0u8;24];
        // A 2x2 BGRA image: 8 active bytes per row, 4 pitch-padding bytes.
        for index in 0..8 {padded[index]=index as u8;}
        for index in 12..20 {padded[index]=(index+2) as u8;}
        let receipt=frame_receipt(nonce,2,2,12,&padded).unwrap();
        padded[8..12].fill(255);
        padded[20..24].fill(255);
        assert_eq!(frame_receipt(nonce,2,2,12,&padded).unwrap(),receipt);
        padded[12]^=1;
        assert_eq!(frame_receipt(nonce,2,2,12,&padded).unwrap(),receipt);
        // All-sensitive screen content is fully suppressed in the receipt.
        padded.fill(255);
        assert_eq!(frame_receipt(nonce,2,2,12,&padded).unwrap(),receipt);
        assert_ne!(frame_receipt("ffeeddccbbaa99887766554433221100",2,2,12,&padded).unwrap(),receipt);
        assert!(frame_receipt(nonce,2,2,12,&padded[..19]).is_none());
        assert!(frame_receipt("not-a-host-nonce",2,2,12,&padded).is_none());
        assert!(frame_receipt(nonce,0,2,12,&padded).is_none());
    }
    #[test]
    fn future_sensitive_region_mask_is_stride_correct_without_touching_padding(){
        let mut frame=vec![99u8;32]; // 3-pixel active BGRA row, 4 pad bytes
        let regions=[SensitiveRect{left:1,top:0,width:1,height:2}];
        assert!(mask_sensitive_bgra_regions(3,2,16,&mut frame,&regions));
        assert_eq!(&frame[4..8],&[0;4]);
        assert_eq!(&frame[20..24],&[0;4]);
        assert_eq!(&frame[0..4],&[99;4]);
        assert_eq!(&frame[8..16],&[99;8]);
        assert_eq!(&frame[24..32],&[99;8]);
    }
    #[test]
    fn reviewed_region_export_defaults_to_black_and_never_copies_pitch_padding(){
        let input=vec![191u8;32]; // 3x2 BGRA, 16 byte pitch, 4 bytes pad
        let region=[ApprovedBgraRect{left:1,top:0,width:1,height:2}];
        let safe=approved_regions_only_bgra(3,2,16,&input,&region).unwrap();
        assert_eq!(&safe[4..8],&[191;4]);
        assert_eq!(&safe[20..24],&[191;4]);
        assert_eq!(&safe[0..4],&[0;4]);
        assert_eq!(&safe[8..20],&[0;12]);
        assert_eq!(&safe[24..32],&[0;8]);
    }
    #[test]
    fn invalid_or_unreviewed_region_cannot_produce_any_pixel_buffer(){
        let input=vec![55u8;32];
        let valid=ApprovedBgraRect{left:1,top:0,width:1,height:2};
        assert!(approved_regions_only_bgra(3,2,16,&input,&[]).is_none());
        for bad in [
            ApprovedBgraRect{left:2,top:0,width:2,height:1},
            ApprovedBgraRect{left:u32::MAX,top:0,width:1,height:1},
            ApprovedBgraRect{left:0,top:0,width:0,height:1},
            ApprovedBgraRect{left:0,top:2,width:1,height:1},
        ]{
            assert!(approved_regions_only_bgra(3,2,16,&input,&[valid,bad]).is_none());
        }
        assert!(approved_regions_only_bgra(3,2,16,&input,&vec![valid;65]).is_none());
    }
    #[test]
    fn invalid_or_untrusted_mask_never_partially_writes_raw_frame(){
        let initial=vec![77u8;32];
        let valid=SensitiveRect{left:1,top:0,width:1,height:2};
        for bad in [
            SensitiveRect{left:u32::MAX,top:0,width:1,height:1},
            SensitiveRect{left:2,top:0,width:2,height:1},
            SensitiveRect{left:0,top:2,width:1,height:1},
            SensitiveRect{left:0,top:0,width:0,height:1}
        ]{
            let mut frame=initial.clone();
            assert!(!mask_sensitive_bgra_regions(3,2,16,&mut frame,&[valid,bad]));
            assert_eq!(frame,initial);
        }
        let mut frame=initial.clone();
        assert!(!mask_sensitive_bgra_regions(3,2,16,&mut frame,&[]));
        assert_eq!(frame,initial);
        let many=vec![valid;4097];
        assert!(!mask_sensitive_bgra_regions(3,2,16,&mut frame,&many));
        assert_eq!(frame,initial);
    }
    #[cfg(windows)]
    #[test]
    fn native_win32_process_birth_is_live_stable_and_query_only(){
        let own_pid=std::process::id();
        let born=windows_capture_worker::process_birth_for_test(own_pid).unwrap();
        assert!(born>0);
        assert_eq!(windows_capture_worker::process_birth_for_test(own_pid).unwrap(),born);
        assert!(windows_capture_worker::process_birth_for_test(0).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn native_foreground_visibility_rejects_absent_hwnd(){
        assert!(windows_capture_worker::visible_window_for_test(0).is_err());
    }
    #[test]
    fn rejects_invalid_dimensions_truncation_and_unbounded_pitch(){
        assert!(!frame_geometry_valid(0,1080,7680,7680*1080));
        assert!(!frame_geometry_valid(7681,1080,7681*4,7681*4*1080));
        assert!(!frame_geometry_valid(1920,1080,300,300*1080));
        assert!(!frame_geometry_valid(1920,1080,7680,100));
        assert!(!frame_geometry_valid(1,1,u32::MAX,usize::MAX));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Lu Graphics Capture worker requires interactive Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
mod windows_capture_worker {
    // A child module does not implicitly import names defined in its parent.
    // Keep the native D3D11 callback and its geometry checks bound to the
    // same validated limits used by the host-independent unit tests.
    use super::{frame_geometry_valid, MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT};
    use std::{env,error::Error,io::{self,Write},time::{Instant,Duration}};
    use windows_capture::{
        capture::{Context,GraphicsCaptureApiHandler},
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        graphics_capture_picker::GraphicsCapturePicker,
        window::Window,
        settings::{
            ColorFormat,CursorCaptureSettings,DirtyRegionSettings,
            DrawBorderSettings,MinimumUpdateIntervalSettings,
            SecondaryWindowSettings,Settings
        },
    };

    const MAX_CAPTURE_SECS:u64=20;

    /// Win32 requires audited query-only API calls to distinguish a genuine
    /// process INSTANCE from a PID recycled after window destruction.
    /// Restricted to this module. Refusal to query does not downgrade to PID.
    #[allow(unsafe_code)]
    mod process_instance {
        use windows::{
            Win32::Foundation::{HANDLE,FILETIME,CloseHandle},
            Win32::System::Threading::{
                OpenProcess,GetProcessTimes,PROCESS_QUERY_LIMITED_INFORMATION
            }
        };
        // Win32 HANDLE is *not* an automatically owning COM/WinRT object.
        // Keep the process-query-only resource within a small RAII boundary.
        struct ProcessQueryHandle(HANDLE);
        impl Drop for ProcessQueryHandle{
            fn drop(&mut self){
                // SAFETY: the handle originated from successful OpenProcess
                // in this scope, is owned exactly once and is not a pseudo
                // handle. Drop does not export the handle to app/renderer.
                let _=unsafe{CloseHandle(self.0)};
            }
        }
        pub(super) fn born_at(pid:u32)->Result<u64,&'static str>{
            if pid==0{return Err("Missing native process identity")}
            // SAFETY: PID originates from the actual OS window; no handles,
            // buffers or pointers are supplied by model/user input. Owned
            // closes the query-only handle on every path. The four pointers
            // point at initialized FILETIME records in this call frame.
            let handle=unsafe{
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid)
            }.map_err(|_|"Cannot attest Windows process instance")?;
            let process=ProcessQueryHandle(handle);
            let mut created=FILETIME::default();
            let mut ended=FILETIME::default();
            let mut kernel=FILETIME::default();
            let mut user=FILETIME::default();
            unsafe{
                GetProcessTimes(process.0,
                    &mut created,&mut ended,&mut kernel,&mut user)
            }.map_err(|_|"Process creation timestamp unavailable")?;
            let instance=(u64::from(created.dwHighDateTime)<<32)
                |u64::from(created.dwLowDateTime);
            if instance==0{return Err("Invalid process creation timestamp")}
            Ok(instance)
        }
    }


    /// OS-backed scope check for an explicitly human-focused WINDOW.
    /// Window title, application path, and visual content are not read.
    /// Only a visible, uncloaked top-level native HWND is eligible.
    #[allow(unsafe_code)]
    mod visible_window {
        use windows::{
            Win32::Foundation::HWND,
            Win32::UI::WindowsAndMessaging::{
                GetAncestor,GA_ROOT,IsWindow,IsWindowVisible
            },
            Win32::Graphics::Dwm::{
                DwmGetWindowAttribute,DWMWA_CLOAKED
            }
        };
        pub(super) fn attested(hwnd:usize)->Result<(),&'static str>{
            if hwnd==0{return Err("Missing top-level OS window identity")}
            let native=HWND(hwnd as *mut std::ffi::c_void);
            // SAFETY: HWND comes from Windows Window::foreground or that same
            // previously selected OS HWND. Read-only OS handle queries.
            if !unsafe{IsWindow(Some(native))}.as_bool()
                || !unsafe{IsWindowVisible(native)}.as_bool()
                || unsafe{GetAncestor(native,GA_ROOT)}!=native {
                return Err("Window is hidden, invalid or not top-level");
            }
            let mut cloak:u32=0;
            // SAFETY: DWM writes exactly four bytes to this owned scalar.
            // Failure is not treated as a visible window.
            unsafe{DwmGetWindowAttribute(
                native,DWMWA_CLOAKED,
                (&mut cloak as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32
            )}.map_err(|_|"Cannot attest desktop compositor visibility")?;
            if cloak!=0{return Err("Window is hidden on this desktop")}
            Ok(())
        }
    }
    #[cfg(test)]
    pub(super) fn visible_window_for_test(hwnd:usize)->Result<(),&'static str>{
        visible_window::attested(hwnd)
    }
    #[cfg(test)]
    pub(super) fn process_birth_for_test(pid:u32)->Result<u64,&'static str>{
        process_instance::born_at(pid)
    }

    struct OneFrame {
        started:Instant,
        captured:bool,
        nonce:String,
        foreground_identity:Option<(usize,u32,u64)>,
    }
    impl GraphicsCaptureApiHandler for OneFrame {
        type Flags=(String,Option<(usize,u32,u64)>);
        type Error=Box<dyn Error+Send+Sync>;
        fn new(ctx:Context<Self::Flags>)->Result<Self,Self::Error>{
            Ok(Self{started:Instant::now(),captured:false,
                nonce:ctx.flags.0,foreground_identity:ctx.flags.1})
        }
        fn on_frame_arrived(
            &mut self,frame:&mut Frame,
            control:InternalCaptureControl
        )->Result<(),Self::Error>{
            if self.captured || self.started.elapsed().as_secs()>MAX_CAPTURE_SECS {
                control.stop();
                return Ok(());
            }
            // The foreground path is bound to a native HWND, PID *and*
            // process creation FILETIME. A recycled PID or HWND is not proof
            // of the originally selected window. The generic OS picker has
            // no such identity and is always marked unverified by the host.
            if let Some((hwnd,pid,born))=self.foreground_identity{
                let window=Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
                if !window.is_valid()||window.process_id()?!=pid
                    ||process_instance::born_at(pid)?!=born
                    ||visible_window::attested(hwnd).is_err(){
                    control.stop();
                    return Err("Foreground window instance changed".into());
                }
            }
            let width=frame.width();
            let height=frame.height();
            if width==0||height==0
                ||width>MAX_FRAME_WIDTH||height>MAX_FRAME_HEIGHT{
                control.stop();
                return Err("Capture dimensions exceed Lu's bounded frame limit".into());
            }
            let mut gpu_frame=frame.buffer()?;
            let row_pitch=gpu_frame.row_pitch();
            let buffer=gpu_frame.as_raw_buffer();
            if !frame_geometry_valid(width,height,row_pitch,buffer.len()){
                control.stop();
                return Err("Invalid mapped D3D11 frame stride or frame size".into());
            }
            // Read WGC/D3D11-mapped frame geometry, then redact the ENTIRE
            // frame for its receipt: no private pixel values reach stdout,
            // the host, a disk file or a model. No useful visual proof exists
            // until separate trusted sensitive-region classification.
            let digest=super::frame_receipt(&self.nonce,width,height,row_pitch,buffer)
                .ok_or("Invalid frame receipt geometry or host nonce")?;
            // FrameBuffer release unmaps staging surface when dropped.
            // The operating system owns the GPU/driver buffers; Lu does not
            // retain or copy the pixel data into a persistent heap or file.
            drop(gpu_frame);
            // Recheck identity AFTER D3D11 readback as well. This closes a
            // meaningful interval where the foreground window or owning
            // process could disappear between the pre-frame check and
            // stdout receipt publication. Still not a durable HWND token.
            if let Some((hwnd,pid,born))=self.foreground_identity{
                let window=Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
                if !window.is_valid()||window.process_id()?!=pid
                    ||process_instance::born_at(pid)?!=born
                    ||visible_window::attested(hwnd).is_err(){
                    control.stop();
                    return Err("Foreground window instance changed during readback".into());
                }
            }
            self.captured=true;
            let mut output=io::stdout().lock();
            writeln!(output,"P\t1\t1")?;
            writeln!(output,"D\t{digest}")?;
            output.flush()?;
            control.stop();
            Ok(())
        }
    }

    pub(super) fn run()->Result<(),Box<dyn Error+Send+Sync>>{
        // A normal user cannot invoke an arbitrary capture command through
        // this binary. Its only operational mode is the host-owned reviewed
        // one-shot picker and it requires interactive OS picker selection.
        let args:Vec<_>=env::args_os().skip(1).collect();
        if args.len()!=2||
            !(args[0]=="--lu-observe-one-shot"
                ||args[0]=="--lu-observe-foreground-one-shot"){
            return Err("Invalid Lu capture worker operation".into());
        }
        let nonce=args[1].to_string_lossy();
        if nonce.len()!=32 ||!nonce.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err("Invalid host-generated capture nonce".into());
        }
        if args[0]=="--lu-observe-foreground-one-shot"{
            // User explicitly approved this delay through the Lu-native
            // confirmation dialog and should focus the target window.
            // This is NOT an automatic window enumeration/capture loop.
            std::thread::sleep(Duration::from_secs(4));
            let window=Window::foreground()?;
            if !window.is_valid(){return Err("Foreground window is not capturable".into());}
            let pid=window.process_id()?;
            if pid==0||pid==std::process::id(){
                return Err("Foreground window must be a separate selected app".into());
            }
            let (width,height)=(window.width()?,window.height()?);
            if width<=0||height<=0
                ||width>MAX_FRAME_WIDTH as i32||height>MAX_FRAME_HEIGHT as i32{
                return Err("Selected window exceeds Lu's size limit".into());
            }
            let born=process_instance::born_at(pid)?;
            visible_window::attested(window.as_raw_hwnd() as usize)?;
            let identity=(window.as_raw_hwnd() as usize,pid,born);
            let settings=Settings::new(
                window,
                CursorCaptureSettings::WithoutCursor,
                DrawBorderSettings::WithBorder,
                SecondaryWindowSettings::Exclude,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                (nonce.into_owned(),Some(identity)),
            );
            return OneFrame::start(settings)
                .map_err(|e|format!("Foreground WGC capture failed: {e:?}").into());
        }
        let Some(picked)=GraphicsCapturePicker::pick_item()? else{
            return Err("Windows capture picker canceled by user".into());
        };
        let (width,height)=picked.size()?;
        if width<=0||height<=0
            ||width>MAX_FRAME_WIDTH as i32||height>MAX_FRAME_HEIGHT as i32{
            return Err("Selected source exceeds Lu's capture size limit".into());
        }
        let settings=Settings::new(
            picked,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithBorder,
            SecondaryWindowSettings::Exclude,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (nonce.into_owned(),None),
        );
        OneFrame::start(settings).map_err(|e|format!("WGC capture failed: {e:?}").into())
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error)=windows_capture_worker::run(){
        // Do not echo OS window titles or sensitive WinRT error metadata;
        // only a coarse failure code is visible in Lu's task ledger.
        let _=error;
        eprintln!("Lu WGC selection or bounded frame capture failed");
        std::process::exit(1);
    }
}
